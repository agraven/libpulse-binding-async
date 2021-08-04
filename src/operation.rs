///! Asyncronous operations
use std::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use libpulse_binding::error::Code;
use pulse::{error::PAErr, operation::State};

/// Asyncronous operation object, representing work being performed by the
/// pulseaudio server.
pub struct Operation<F: ?Sized>(pulse::operation::Operation<F>);

impl<F: ?Sized> Future for Operation<F> {
	type Output = Result<(), PAErr>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match self.0.get_state() {
			State::Done => return Poll::Ready(Ok(())),
			State::Cancelled => return Poll::Ready(Err(Code::Killed.into())),
			_ => (),
		}
		let waker = cx.waker().clone();
		self.get_mut().0.set_state_callback(Some(Box::new(move || {
			waker.wake_by_ref();
		})));
		Poll::Pending
	}
}

impl<F: ?Sized> From<pulse::operation::Operation<F>> for Operation<F> {
	fn from(op: pulse::operation::Operation<F>) -> Self {
		Operation(op)
	}
}
