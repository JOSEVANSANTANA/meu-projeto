fn main() {
    // O código compartilhado com o app Tauri usa os cfgs `desktop`/`mobile`
    // (definidos pelo tauri-build). No servidor, somos "desktop" (há spawn de
    // subprocesso disponível), então definimos o cfg manualmente.
    println!("cargo:rustc-check-cfg=cfg(desktop)");
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    println!("cargo:rustc-cfg=desktop");
}
