fn main() {
    println!("cargo:rerun-if-changed=assets/deskcat.ico");
    #[cfg(windows)]
    embed_windows_resources();
}

#[cfg(windows)]
fn embed_windows_resources() {
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("assets/deskcat.ico").exists() {
        res.set_icon("assets/deskcat.ico");
    }
    res.set("ProductName", "DeskCat");
    res.set("FileDescription", "DeskCat — desktop typing companion");
    res.set("LegalCopyright", "MIT License");
    if let Err(e) = res.compile() {
        // missing rc.exe etc. should not break the build, just skip branding
        println!("cargo:warning=resource embedding skipped: {e}");
    }
}
