#[cfg(windows)]
fn main() {
    // Only embed icon for the CLI binary; the GUI binary gets its resource
    // from tauri-build in src-tauri/.
    if std::env::var("CARGO_BIN_NAME").as_deref() != Ok("fastzip-cli") {
        return;
    }
    if let Err(error) = winres::WindowsResource::new()
        .set_icon("assets/fastzip.ico")
        .compile()
    {
        println!("cargo:warning=Failed to embed icon resource: {error}");
    }
}

#[cfg(not(windows))]
fn main() {}
