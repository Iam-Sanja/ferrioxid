//! Kopiert das gebaute Admin-Frontend in ein Verzeichnis, das der Compiler einbettet.
//! Node.js wird zur Laufzeit nicht benötigt.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist = manifest.join("../../apps/admin-ui/dist");
    let embedded = manifest.join("embedded-admin");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../apps/admin-ui/dist");
    if embedded.exists() {
        let _ = std::fs::remove_dir_all(&embedded);
    }
    std::fs::create_dir_all(&embedded).expect("embedded-admin");
    if dist.join("index.html").is_file() {
        copy_dir(&dist, &embedded);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let dest = to.join(entry.file_name());
        if kind.is_dir() {
            let _ = std::fs::create_dir_all(&dest);
            copy_dir(&entry.path(), &dest);
        } else if kind.is_file() {
            let _ = std::fs::copy(entry.path(), dest);
        }
    }
}
