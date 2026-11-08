//! Die Admin-Oberfläche wird aus dem Quellbaum eingebettet. Node.js wird nicht aufgerufen.

fn main() {
    println!("cargo:rerun-if-changed=templates");
    println!("cargo:rerun-if-changed=static");
}
