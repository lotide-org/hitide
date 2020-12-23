# hitide installation
Requirements: rustc & cargo, openssl, lotide

Set these environment variables:
 - BACKEND_HOST - URL path to lotide, for example `http://localhost:3333`.
 - PORT (optional) - Port number to bind to. Defaults to 4333.

To build hitide, run `cargo build --release`. A `hitide` binary will appear in `./target/release`.
