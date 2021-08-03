//! An `async`/`await` wrapper for `libpulse-binding` intended to remove the
//! ergonomics issues libpulse-binding's callback-oriented API imposes.

#![warn(
	missing_docs,
	trivial_casts,
	trivial_numeric_casts,
	unused_extern_crates,
	unused_import_braces,
	unused_qualifications,
	dead_code,
	clippy::unwrap_used,
	clippy::expect_used
)]

extern crate libpulse_binding as pulse;

pub mod context;
//pub mod mainloop;
pub mod operation;
