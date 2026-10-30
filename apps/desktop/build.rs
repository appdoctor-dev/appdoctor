//! Gives the Windows executable AppDoctor's icon. macOS takes its icon from
//! the .app bundle (scripts/package-macos.sh), so this does nothing there.

fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon/AppIcon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app-icon/AppIcon.ico");
        resource.compile().expect("the Windows icon embeds");
    }
}
