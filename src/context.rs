//! Connection context

use std::{
	pin::Pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	task::Poll,
};

use libpulse_binding::{
	context::{FlagSet, State},
	def::SpawnApi,
	error::PAErr,
	mainloop::api::Mainloop,
	proplist::{Proplist, UpdateMode},
	sample::Spec,
	volume::Volume,
};

use crate::operation::Operation;

struct ConnectFuture<'a> {
	context: &'a mut pulse::context::Context,
	server: Option<&'a str>,
	flags: FlagSet,
	api: Option<&'a SpawnApi>,
}

impl<'a> std::future::Future for ConnectFuture<'a> {
	type Output = Result<(), PAErr>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
		if self.context.get_state().is_good() {
			return Poll::Ready(Ok(()));
		}
		let waker = cx.waker().clone();

		self.context.set_state_callback(Some(Box::new(move || {
			waker.wake_by_ref();
		})));
		let server = self.server.take();
		let flags = self.flags;
		let api = self.api.take();
		if let Err(err) = self.context.connect(server, flags, api) {
			return Poll::Ready(Err(err));
		}
		Poll::Pending
	}
}

/// An opaque connection context to a daemon
pub struct Context(pulse::context::Context);

impl Context {
	/// Constructs a new connection context.
	///
	/// It's recommended to use [`new_with_proplist()`] instead to specify some
	/// initial client properties.
	///
	/// Note that the `Context` must first be connected with `.connect()`
	pub fn new(mainloop: &impl Mainloop, name: &str) -> Option<Context> {
		pulse::context::Context::new(mainloop, name).map(Context)
	}

	/// Constructs a new connection context with the given client properties.
	///
	/// Note that the `Context` must first be connected with `.connect()`
	pub fn new_with_proplist(
		mainloop: &impl Mainloop,
		name: &str,
		proplist: &Proplist,
	) -> Option<Context> {
		pulse::context::Context::new_with_proplist(mainloop, name, proplist).map(Context)
	}

	/// Checks if some data is pending to be written to the connection
	pub fn is_pending(&self) -> bool {
		self.0.is_pending()
	}

	/// Gets the current context status.
	pub fn get_state(&self) -> State {
		self.0.get_state()
	}

	/// Connects the context to the specified server.
	///
	/// If server is `None`, connect to the default server. This routine may but
	/// will not always return synchronously on error. Use
	/// [`set_state_callback()`] to be notified when the connection is
	/// established. If `flags` doesn’t have [`FlagSet::NOAUTOSPAWN`] set and no
	/// specific server is specified or accessible, a new daemon is spawned. If
	/// `api` is not `None`, the functions specified in the structure are used
	/// when forking a new child process.
	///
	/// [`set_state_callback()`]: Self::set_state_callback
	pub async fn connect(
		&mut self,
		server: Option<&str>,
		flags: FlagSet,
		api: Option<&SpawnApi>,
	) -> Result<(), PAErr> {
		ConnectFuture { context: &mut self.0, server, flags, api }.await
	}

	/// Terminates the context connection immediately
	pub fn disconnect(&mut self) {
		self.0.disconnect()
	}

	/// Removes a sample from the sample cache.
	pub async fn remove_sample(&mut self, name: &str) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.remove_sample(name, move |suc| success.store(suc, Ordering::Release)).into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Plays a sample from the sample cache to the specified device.
	///
	/// If the specified device is `None` use the default sink.
	///
	/// # Params
	///
	/// * `name`: Name of the sample to play.
	/// * `dev`: Sink to play this sample on, or `None` for default.
	/// * `volume`: Volume to play this sample with, or `None` to leave the
	///   decision about the volume to the server side which is a good idea.
	///   [`Volume::INVALID`] has the same meaning as `None.
	pub async fn play_sample(
		&mut self,
		name: &str,
		dev: Option<&str>,
		volume: Option<Volume>,
	) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0
				.play_sample(
					name,
					dev,
					volume,
					Some(Box::new(move |suc| success.store(suc, Ordering::Release))),
				)
				.into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Plays a sample from the sample cache to the specified device, allowing
	/// specification of a property list for the playback stream.
	///
	/// If the device is `None` use the default sink.
	///
	/// # Params
	///
	/// * `name`: Name of the sample to play.
	/// * `dev`: Sink to play this sample on, or `None` for default.
	/// * `volume`: Volume to play this sample with, or `None` to leave the
	///   decision about the volume to the server side which is a good idea.
	///   [`Volume::INVALID`] has the same meaning as `None.
	/// * `proplist`: Property list for this sound. The property list of the
	///   cached entry will have this merged into it.
	pub async fn play_sample_with_proplist(
		&mut self,
		name: &str,
		dev: Option<&str>,
		volume: Option<Volume>,
		proplist: &Proplist,
	) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0
				.play_sample_with_proplist(
					name,
					dev,
					volume,
					proplist,
					Some(Box::new(move |suc| success.store(suc.is_ok(), Ordering::Release))),
				)
				.into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Tells the daemon to exit.
	///
	/// This function is unlikely to be successfully driven to completion, as
	/// the daemon is likely to exit before a success notification can be sent.
	/// It's therefore recommended to cancel it after a timeout
	///
	/// # Panics
	/// Panics if the underlying C function returns a null pointer
	pub async fn exit_daemon(&mut self) -> bool {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.exit_daemon(move |suc| success.store(suc, Ordering::Release)).into()
		};
		if op.await.is_err() {
			return false;
		}
		success.load(Ordering::Acquire)
	}

	/// Set the default sink to the one with the given name.
	///
	/// # Panics
	/// Panics if the underlying C function returns a null pointer
	pub async fn set_default_sink(&mut self, name: &str) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.set_default_sink(name, move |suc| success.store(suc, Ordering::Release)).into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Set the default source to the one with the given name.
	///
	/// # Panics
	/// Panics if the underlying C function returns a null pointer
	pub async fn set_default_source(&mut self, name: &str) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.set_default_source(name, move |suc| success.store(suc, Ordering::Release)).into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Sets a different application name for context on the server.
	pub async fn set_name(&mut self, name: &str) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.set_name(name, move |suc| success.store(suc, Ordering::Release)).into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Updates the property list of the client, adding new entries.
	pub async fn proplist_update(
		&mut self,
		mode: UpdateMode,
		proplist: &Proplist,
	) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0
				.proplist_update(mode, proplist, move |suc| success.store(suc, Ordering::Release))
				.into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Removes the entries with the given names from the client's property list
	pub async fn proplist_remove(&mut self, keys: &[&str]) -> Result<(), PAErr> {
		let success = Arc::new(AtomicBool::new(false));
		let op: Operation<_> = {
			let success = Arc::clone(&success);
			self.0.proplist_remove(keys, move |suc| success.store(suc, Ordering::Release)).into()
		};
		op.await?;
		match success.load(Ordering::Acquire) {
			false => Err(self.0.errno()),
			true => Ok(()),
		}
	}

	/// Checks if this is a connection to a local daemon.
	///
	/// Returns `true` when the connection is to a local daemon. Returns `None`
	/// on error, for instance when no connection has been made yet.
	pub fn is_local(&self) -> Option<bool> {
		self.0.is_local()
	}

	/// Gets the name of the server this context is connected to.
	pub fn get_server(&self) -> Option<String> {
		self.0.get_server()
	}

	/// Gets the protocol version of the library
	pub fn get_protocol_version(&self) -> u32 {
		self.0.get_protocol_version()
	}

	/// Gets the protocal version of the server this context is connected to.
	///
	/// Returns `None` on error.
	pub fn get_server_protocol_version(&self) -> Option<u32> {
		self.0.get_server_protocol_version()
	}

	/// Gets the client index this context is identified in the server with.
	///
	/// This is useful for usage with the introspection functions, such as
	/// `Introspector::get_client_info()`.
	///
	/// Returns `None` on error.
	pub fn get_index(&self) -> Option<u32> {
		self.0.get_index()
	}

	/// Gets the optimal block size for passing around audio buffers.
	///
	/// It is recommended to allocate buffers of the size returned here when
	/// writing audio data to playback streams, if the latency constraints
	/// permit this. It is not recommended writing larger blocks than this
	/// because usually they will then be split up internally into chunks of
	/// this size. It is not recommended writing smaller blocks than this
	/// (unless required due to latency demands) because this increases CPU
	/// usage.
	///
	/// If `ss` is `None` you will be returned the byte-exact tile size.
	///
	/// If `ss` is invalid, returns `None`, else returns tile size rounded down
	/// to multiple of the frame size.
	///
	/// This is supposed to be used in a construct such as:
	///
	/// ```rust,ignore
	/// let ss = stream.get_sample_spec().unwrap();
	/// let size = context.get_tile_size(Some(ss)).unwrap();
	/// ```
	pub fn get_tile_size(&self, ss: Option<&Spec>) -> Option<usize> {
		self.0.get_tile_size(ss)
	}

	// TODO: load_cookie_from file
	// TODO: rttime_new
}
