//! On Windows, embed the app icon and version details in fanimate-cli.exe, so
//! Explorer shows Fanimate's icon.

fn main() {
    println!("cargo:rerun-if-changed=../../brand/app-icon/fanimate.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../brand/app-icon/fanimate.ico")
        .set("ProductName", "Fanimate")
        .set("FileDescription", "Fanimate command-line tool")
        .set("CompanyName", "The Fanimate Contributors")
        .set("LegalCopyright", "The Fanimate Contributors. MIT or Apache 2.0.");
    if let Err(e) = res.compile() {
        // a build without the icon still works; say why it is missing
        println!("cargo:warning=could not embed the Windows icon: {e}");
    }
}
