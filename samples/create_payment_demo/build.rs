//! modifies build to dynamically link in a) indy-sdk

use std::env;

fn main() {

    let libindy_lib_path = env::var("LIBINDY_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}",libindy_lib_path);
    println!("cargo:rustc-link-search=native=../../libsovtoken/target/debug/");

    if let Ok(mode) = env::var("LIBINDY_STATIC") {
        println!("cargo:rustc-link-lib=static=indy");
        println!("cargo:rustc-link-lib=static=sovtoken");
    } else {
        println!("cargo:rustc-link-lib=dylib=indy");
        println!("cargo:rustc-link-lib=dylib=sovtoken");
    }

    let target = env::var("TARGET").unwrap();
    println!("target={}", target);

    if target.find("-windows-").is_some() {
        println!("cargo:rustc-link-lib=dylib=ssleay32");
        println!("cargo:rustc-link-lib=dylib=zmq");
        println!("cargo:rustc-link-lib=dylib=sodium");
        let prebuilt_dir = env::var("INDY_PREBUILT_DEPS_DIR").unwrap();
        println!("cargo:rustc-flags=-L {}\\lib", prebuilt_dir);
        return;
    }
}
