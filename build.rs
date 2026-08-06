use std::env;
use std::path::PathBuf;

fn main() {
    // Get the manifest directory (project root)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_rc = manifest_dir.join("assets").join("url-ferry.rc");

    // Verify the resource file exists
    if !icon_rc.exists() {
        eprintln!("Warning: Resource file not found at {:?}", icon_rc);
        println!("cargo:rerun-if-changed=assets/url-ferry.rc");
        println!("cargo:rerun-if-changed=assets/url-ferry.ico");
        return;
    }

    // Tell cargo to rerun this script if either the .rc or .ico files change
    println!("cargo:rerun-if-changed=assets/url-ferry.rc");
    println!("cargo:rerun-if-changed=assets/url-ferry.ico");

    // Use embed-resource crate to compile the Windows resource file
    // This will embed the icon in both binaries (listener and sender)
    // Pass empty vec for macros
    embed_resource::compile(&icon_rc, Vec::<String>::new().into_iter());
}
