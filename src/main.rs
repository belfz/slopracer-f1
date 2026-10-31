//! Slopracer F1 in the terminal. The browser build starts from `web.rs` instead.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    slopracer_f1::term::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
