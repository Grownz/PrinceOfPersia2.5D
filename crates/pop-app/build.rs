// Embeds assets/icon.ico in the exe (Explorer, taskbar, window title bar). Needs rc.exe (Windows SDK) or windres.
fn main() {
    #[cfg(windows)]
    winresource::WindowsResource::new().set_icon("../../assets/icon.ico").compile().expect("embed the app icon");
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
}
