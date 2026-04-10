use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir should exist"));
    let glue_dir = manifest_dir.parent().expect("host-smoke should live under driver/glue");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should exist"));
    let source = glue_dir.join("wd_runtime_host_smoke.c");
    let header = glue_dir.join("wd_kmdf_bridge.h");
    let object = out_dir.join("wd_runtime_host_smoke.obj");
    let library = out_dir.join("wd_runtime_host_smoke.lib");
    let clang = env::var("CLANG_CL").unwrap_or_else(|_| "clang-cl".to_string());
    let llvm_lib = env::var("LLVM_LIB").unwrap_or_else(|_| "llvm-lib".to_string());

    run(
        Command::new(clang)
            .arg("/nologo")
            .arg("/TC")
            .arg("/c")
            .arg(format!("/I{}", glue_dir.display()))
            .arg(format!("/Fo{}", object.display()))
            .arg(&source),
        "compile host smoke C source",
    );

    run(
        Command::new(llvm_lib)
            .arg("/nologo")
            .arg(format!("/out:{}", library.display()))
            .arg(&object),
        "archive host smoke object library",
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=wd_runtime_host_smoke");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-env-changed=CLANG_CL");
    println!("cargo:rerun-if-env-changed=LLVM_LIB");
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|err| panic!("failed to {action}: {err}"));
    assert!(status.success(), "{action} exited with status {status}");
}
