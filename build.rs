fn main() {
    // Generate bindings for headers listed in kernel-wrapper.h.
    println!("cargo:rerun-if-changed=src/kernel_headers.h");
    let uname = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .unwrap()
        .stdout;
    let kernel_version = std::str::from_utf8(uname.as_slice()).unwrap();
    let bindings = bindgen::Builder::default()
        .header("src/kernel_headers.h")
        .clang_arg(format!("-I/lib/modules/{}/build/include", kernel_version))
        .derive_debug(true)
        .derive_default(true)
        .rustified_enum("*")
        .generate()
        .expect("Unable to generate bindings");
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("kernel_headers.rs"))
        .expect("Couldn't write bindings!");

    // Compile asm helpers file into the rust library.
    println!("cargo:rerun-if-changed=src/asm_helpers.c");
    cc::Build::new()
        .file("src/asm_helpers.c")
        .flag("-O3")
        .warnings_into_errors(true)
        .compile("asm_helper");
}
