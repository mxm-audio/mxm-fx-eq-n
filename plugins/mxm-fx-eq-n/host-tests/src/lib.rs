//! mxm-fx-eq-n hosted by MXM Player. The tests are in `tests/` and load the plugin's release bundle
//! from `target/bundled`, skipping with the command to build it when it is missing:
//! `cargo xtask bundle mxm-fx-eq-n --release` and `cargo xtask fetch`, then
//! `cargo test -p mxm-fx-eq-n-host-tests`.
