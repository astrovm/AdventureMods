//! Embed the game covers and mod screenshots, keyed by the resource paths the
//! mod catalogs use.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const RESOURCE_PREFIX: &str = "/io/github/astrovm/AdventureMods/resources";

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let data = manifest_dir.join("data");
    let mut files = Vec::new();
    collect(&data.join("resources/images"), &mut files);
    collect(&data.join("covers"), &mut files);
    files.sort();

    let mut out = String::from("pub(crate) static ASSETS: &[(&str, &[u8])] = &[\n");
    for file in &files {
        let key = if let Ok(relative) = file.strip_prefix(data.join("resources")) {
            format!("{RESOURCE_PREFIX}/{}", relative.display())
        } else {
            let relative = file.strip_prefix(&data).unwrap();
            format!("{RESOURCE_PREFIX}/{}", relative.display())
        };
        writeln!(out, "    ({key:?}, include_bytes!({:?})),", file.display()).unwrap();
    }
    out.push_str("];\n");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("assets.rs"), out).unwrap();
    println!("cargo:rerun-if-changed=data/resources/images");
    println!("cargo:rerun-if-changed=data/covers");
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "jpg" || ext == "png")
        {
            files.push(path);
        }
    }
}
