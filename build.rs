fn generate_kernel_bindings() {
    let kernel_header_wrapper = "src/perf/kernel_headers.h";
    println!("cargo:rerun-if-changed={}", kernel_header_wrapper);
    let bindings = bindgen::Builder::default()
        .header(kernel_header_wrapper)
        .derive_debug(true)
        .impl_debug(true)
        .derive_default(true)
        .rustified_enum(r".*")
        .whitelist_type(r"^perf_.*")
        .whitelist_var(r"^PERF_.*")
        .blacklist_type(r"^__kernel.*")
        .generate()
        .expect("Unable to generate bindings");
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("kernel_headers.rs"))
        .expect("Couldn't write bindings!");
}

fn compile_asm_helpers() {
    let asm_helpers = if cfg!(target_arch = "x86_64") {
        "src/arch/x86_64/asm_helpers.c"
    } else if cfg!(target_arch = "powerpc64") {
        "src/arch/powerpc64/asm_helpers.c"
    } else {
        return;
    };
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
