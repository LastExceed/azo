# azo

A library for interacting with ASIO (Audio Stream Input/Output) drivers.

### Not an `ASIO SDK` Wrapper

For Rust bindings to the official [ASIO SDK by Steinberg](https://www.steinberg.net/developers/prorietary-sdk/), see the [`asio-sys`](https://crates.io/crates/asio-sys) crate instead.

`azo` doesn't use the SDK, it accesses the underlying COM objects exposed by the drivers directly.

### Getting Started

`azo::get_drivers()` is always the starting point, followed by `.create_instance()` on one (or multiple) of the returned structs. See the `/examples` provided.