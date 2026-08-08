# azo

is a multi-driver capable re-implementation of Steinberg's ASIO (Audio Stream Input/Output) protocol.

### Not an `ASIO SDK` Wrapper

If you are looking for Rust bindings to the official [ASIO SDK by Steinberg](https://www.steinberg.net/developers/prorietary-sdk/), take a look at the [`asio-sys`](https://crates.io/crates/asio-sys) crate instead.

`azo` doesn't use the SDK, it instead directly interacts with the underlying COM objects exposed by the drivers.

### Getting Started

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

## On The Legality of `azo`'s License

A common concern about re-implementations of ASIO is whether they are actually legally allowed to do so without assuming the license of the ASIO SDK. The short answer is yes. The long answer is:

Both Steinberg and the author of `azo` (LastExceed) reside in germany, and german copyright law (Urheberrechtsgesetz) very clearly protects this kind of project under the umbrella of "interoperability". The relevant paragraph is [UrhG § 69](https://www.gesetze-im-internet.de/urhg/__69.html) (nice), specifically the following sections:

* [UrhG § 69a(2)](https://www.gesetze-im-internet.de/urhg/__69a.html) declares foundational APIs essentially uncopyrightable

* [UrhG § 69d(3)](https://www.gesetze-im-internet.de/urhg/__69d.html) permits studying the SDK without permission from Steinberg

* [UrhG § 69g(2)](https://www.gesetze-im-internet.de/urhg/__69g.html) nullifies any contradicting clauses that the SDK's license agreement might contain

Steinberg has indeed taken legal action against open source projects in the past, but this was in response to people effectively redistributing the VST2 SDK verbatim, which is very different from what `azo` does. Other typical pitfalls include the improper re-use of trademarked names (such as "ASIO" in [FlexASIO](https://github.com/dechamps/FlexASIO), which is why that particular project was forced to add a disclaimer), or copying documentation, which `azo` doesn't do either.