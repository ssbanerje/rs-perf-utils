fn generate_kernel_bindings() {
    let uname = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .unwrap()
        .stdout;
    let kernel_version = std::str::from_utf8(uname.as_slice()).unwrap();
    let kernel_header_wrapper = "src/kernel_headers.h";
    println!("cargo:rerun-if-changed={}", kernel_header_wrapper);
    let bindings = bindgen::Builder::default()
        .header(kernel_header_wrapper)
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
}

fn compile_asm_helpers() {
    let asm_helpers = "src/arch/asm_helpers.c";
    println!("cargo:rerun-if-changed={}", asm_helpers);
    cc::Build::new()
        .file(asm_helpers)
        .flag("-O3")
        .warnings_into_errors(true)
        .compile("asm_helper");
}

fn main() {
    // Generate bindings for headers listed in kernel-wrapper.h.
    generate_kernel_bindings();

    // Compile asm helpers file into the rust library.
    compile_asm_helpers();
}
