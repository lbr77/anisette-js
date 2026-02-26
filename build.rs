use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=UNICORN_DIR");
    println!("cargo:rerun-if-env-changed=UNICORN_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=UNICORN_INCLUDE_DIR");

    let target = env::var("TARGET").unwrap_or_default();
    if target != "wasm32-unknown-emscripten" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let unicorn_dir = env::var("UNICORN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("../unicorn"));
    let unicorn_build_dir = env::var("UNICORN_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| unicorn_dir.join("build"));
    let unicorn_include_dir = env::var("UNICORN_INCLUDE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| unicorn_dir.join("include"));

    let required_libs = [
        "libunicorn.a",
        "libunicorn-common.a",
        "libaarch64-softmmu.a",
        "libarm-softmmu.a",
    ];
    for lib in required_libs {
        let path = unicorn_build_dir.join(lib);
        if !path.exists() {
            panic!(
                "missing unicorn static library: {}. run `bash test/rebuild-unicorn.sh` first",
                path.display()
            );
        }
    }

    println!("cargo:rustc-link-arg=--no-entry");
    println!("cargo:rustc-link-arg=-sSTANDALONE_WASM=1");
    println!(
        "cargo:rustc-link-search=native={}",
        unicorn_build_dir.display()
    );
    if !unicorn_include_dir.exists() {
        println!(
            "cargo:warning=unicorn include dir not found at {}",
            unicorn_include_dir.display()
        );
    }

    for lib in [
        "unicorn",
        "unicorn-common",
        "aarch64-softmmu",
        "arm-softmmu",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}
