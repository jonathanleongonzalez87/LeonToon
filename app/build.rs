//! Incrusta el icono del programa (assets/icon.ico) en el .exe de Windows para que lo muestre el Explorador.
//! Usa `windres` (MinGW). Si no lo encuentra, avisa y el programa se compila igual, solo sin icono de archivo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_windres() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WINDRES") {
        return Some(PathBuf::from(p));
    }
    // En el PATH
    if Command::new("windres").arg("--version").output().map_or(false, |o| o.status.success()) {
        return Some(PathBuf::from("windres"));
    }
    // WinLibs instalado con winget
    let base = std::env::var("LOCALAPPDATA").ok()?;
    let pk = Path::new(&base).join("Microsoft/WinGet/Packages");
    for e in std::fs::read_dir(pk).ok()?.flatten() {
        if e.file_name().to_string_lossy().starts_with("BrechtSanders.WinLibs") {
            let c = e.path().join("mingw64/bin/windres.exe");
            if c.exists() {
                return Some(c);
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let Some(windres) = find_windres() else {
        println!("cargo:warning=No encuentro windres: el .exe se compila sin icono de archivo.");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("icon.o");
    let status = Command::new(&windres).current_dir("assets").args(["icon.rc", "-O", "coff", "-o"]).arg(&out).status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", out.display()),
        _ => println!("cargo:warning=windres falló: el .exe se compila sin icono de archivo."),
    }
}
