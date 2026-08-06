use std::env;
use std::fs;
use std::path::PathBuf;

fn parse_version_components(version: &str) -> (u32, u32, u32, u32) {
    let core = version.split('-').next().unwrap_or(version);
    let mut nums = core.split('.').filter_map(|p| p.parse::<u32>().ok());

    let major = nums.next().unwrap_or(0);
    let minor = nums.next().unwrap_or(0);
    let patch = nums.next().unwrap_or(0);
    let build = 0;

    (major, minor, patch, build)
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let icon_path = manifest_dir.join("assets").join("url-ferry.ico");
    let rc_template_path = manifest_dir.join("assets").join("url-ferry.rc");

    println!("cargo:rerun-if-changed=assets/url-ferry.rc");
    println!("cargo:rerun-if-changed=assets/url-ferry.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    if !rc_template_path.exists() {
        eprintln!("Warning: rc template file not found at {:?}", rc_template_path);
        return;
    }

    if !icon_path.exists() {
        eprintln!("Warning: icon file not found at {:?}", icon_path);
        return;
    }

    let pkg_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION missing");
    let (major, minor, patch, build) = parse_version_components(&pkg_version);
    let icon_path_escaped = icon_path.to_string_lossy().replace('\\', "\\\\");
    let version_numeric = format!("{major},{minor},{patch},{build}");
    let version_string = format!("{major}.{minor}.{patch}.{build}");
    let rc_template = fs::read_to_string(&rc_template_path).expect("Failed to read rc template");

    let rc_contents = rc_template
        .replace("{{ICON_PATH}}", &icon_path_escaped)
        .replace("{{FILE_VERSION_NUM}}", &version_numeric)
        .replace("{{PRODUCT_VERSION_NUM}}", &version_numeric)
        .replace("{{FILE_VERSION_STR}}", &version_string)
        .replace("{{PRODUCT_VERSION_STR}}", &version_string);

    let generated_rc = out_dir.join("url-ferry-generated.rc");
    fs::write(&generated_rc, rc_contents).expect("Failed to write generated rc");

    embed_resource::compile(&generated_rc, Vec::<String>::new().into_iter());
}
