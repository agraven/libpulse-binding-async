# libpulse-async

**NOTICE**: This project is not maintained and not in a useable state

`libpulse-async` is a wrapper for the crate `libpulse-binding` providing an
`async`/`await` based interface. The callback oriented API in `libpulse-binding`
which is imposed by the structure of the pulseaudio C library can get quite
tedious to work with. This crate aims to solve some of those ergonomics issues
with its async interface.
