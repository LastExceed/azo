# azo

A library for interacting with ASIO (Audio Stream Input/Output) drivers.

[![](https://img.shields.io/crates/v/azo?label=crates.io%20(azo))](https://crates.io/crates/azo) [![](https://img.shields.io/crates/v/azo-sys?label=crates.io%20(azo-sys))]((https://crates.io/crates/azo-sys)) ![](https://img.shields.io/crates/d/azo)

### Not an `ASIO SDK` Wrapper

For Rust bindings to the official [ASIO SDK by Steinberg](https://www.steinberg.net/developers/prorietary-sdk/), see the [`asio-sys`](https://crates.io/crates/asio-sys) crate instead.

`azo` does not use the SDK, it instead accesses the underlying COM objects exposed by the drivers directly.

### Cargo Features

* `undocumented` - Include message selectors not mentioned in the ASIO specification

* `host` - Use this if you are not familiar with COM, but
  * care about correctness a lot
  * need to access driver instances from multiple threads

### Getting Started

1. Discover drivers registered in the system via `driver::Metadata::enumerate()`
2. Instanciate the driver(s) via `driver::SafeHandle::new()`
3. Initialize the driver instance via `.init()`
4. ???
5. Profit.

See also the `/examples` provided.