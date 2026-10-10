fn main() {
    embed_icon();
}

#[cfg(target_os = "windows")]
fn embed_icon() {
    if let Err(e) = winresource::WindowsResource::new().set_icon_with_id("assets/icon.ico", "1").compile() {
        println!("cargo:warning=failed to embed icon: {e}");
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
}

#[cfg(not(target_os = "windows"))]
fn embed_icon() {}
