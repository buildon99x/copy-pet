fn main() {
    println!("cargo:rerun-if-changed=assets/clipcat.ico");
    #[cfg(windows)]
    embed_windows_resources();
}

#[cfg(windows)]
fn embed_windows_resources() {
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("assets/clipcat.ico").exists() {
        res.set_icon("assets/clipcat.ico");
    }
    res.set("ProductName", "ClipCat");
    res.set("FileDescription", "ClipCat — desktop cat clipboard manager");
    res.set("LegalCopyright", "MIT License");
    if let Err(e) = res.compile() {
        // missing rc.exe etc. should not break the build, just skip branding
        println!("cargo:warning=resource embedding skipped: {e}");
    }
}
