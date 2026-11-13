//! On Windows: put the PrintZilla icon and name into the .exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/printzilla.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/printzilla.ico");
        res.set("ProductName", "PrintZilla");
        res.set("FileDescription", "PrintZilla – Bibliothek für 3D-Druck-Dateien");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Programmsymbol konnte nicht eingebettet werden: {e}");
        }
    }
}
