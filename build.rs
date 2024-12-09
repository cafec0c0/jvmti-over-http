use std::env;
use std::path::PathBuf;

fn main() {
    let java_home = env::var("JAVA_HOME")
        .expect("JAVA_HOME was not set, unable to locate headers for binding generation");
    let out_dir = env::var("OUT_DIR").unwrap();

    let include_path = PathBuf::from(java_home).join("include");

    let native_include_path = PathBuf::from(&include_path).join(env::consts::OS);

    let include_dirs = [
        include_path.to_str().unwrap(),
        native_include_path.to_str().unwrap(),
    ]
        .map(|d| format!("-I{}", d));

    bindgen::Builder::default()
        .clang_args(include_dirs)
        .header("wrapper.h")
        .blocklist_function("Agent_OnLoad") // Defined by user
        .blocklist_function("Agent_OnAttach") // Defined by user
        .blocklist_function("Agent_OnUnload") // Defined by user
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(PathBuf::from(out_dir).join("jvmti.rs"))
        .expect("Unable to write bindings");
}
