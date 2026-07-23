# azo

is a re-implementation of Steinberg's ASIO (Audio Stream Input/Output) protocol.

### Not an `ASIO SDK` wrapper

If you are looking for Rust bindings to the official [ASIO SDK by Steinberg](https://www.steinberg.net/developers/prorietary-sdk/), take a look at the [`asio-sys`](https://crates.io/crates/asio-sys) crate instead.

`azo` doesn't use the SDK, it instead directly interacts with the underlying COM objects exposed by the drivers.

### Multi-driver support

With `azo` you can load and interact with multiple drivers at the same time, without limitations.

### Windows only

Technically, ASIO is platform agnostic. However, the spec does not define how drivers are to be discovered on other platforms, and is in general very Windows centric, which is why `azo` is (at this time) exclusive to Windows.

### Getting started

from `examples/hello_world.rs`:
```rust
fn main() {
    let all = azo::discover_drivers().unwrap();
    let driver = all[0].create_instance().unwrap();

    driver.init(None).unwrap();
    let rate = driver.get_sample_rate().unwrap();

    println!("current sample rate: {rate}");
}
```
example output:
> current sample rate: 44100

More `/examples` are available.