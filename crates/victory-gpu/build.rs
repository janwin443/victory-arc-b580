use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let target = env::var("TARGET").unwrap_or_default();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));

    if !target.contains("windows") {
        return;
    }

    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("victory-gpu must remain under crates/");
    let dxc_dir = workspace_root.join("third-party/dxc/bin/x64");
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("Cargo OUT_DIR layout changed; cannot locate target profile directory");

    for name in ["dxcompiler.dll", "dxil.dll"] {
        let source = dxc_dir.join(name);
        let destination = profile_dir.join(name);
        println!("cargo:rerun-if-changed={}", source.display());

        if !source.is_file() {
            panic!(
                "DXC runtime is missing: {}. Put the official x64 DXC runtime DLLs in third-party/dxc/bin/x64/",
                source.display()
            );
        }

        fs::copy(&source, &destination).unwrap_or_else(|error| {
            panic!(
                "could not stage {} beside the Windows executable at {}: {error}",
                source.display(),
                destination.display()
            )
        });
    }
}
