# hitide installation
Requirements: nightly rustc & cargo, openssl, lotide

Set these environment variables:
 - BACKEND_HOST - URL path to lotide
 - PORT (optional) - Port number to bind to. Defaults to 4333.

To build hitide, run `cargo build --release` (note that this currently requires a nightly build of rust, hopefully should be resolved soonish). A `hitide` binary will appear in `./target/release`.
