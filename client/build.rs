use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=java/AnemoiaInterceptor.java");
    println!("cargo:rerun-if-changed=java/netty_stubs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Compiled against local stubs — no real Netty jar needed at build time.
    // MC's classloader resolves the real Netty at runtime when we defineClass.
    let result = Command::new("javac")
        .args(["--release", "8"])
        .arg("-d")
        .arg(&out_dir)
        .arg("-sourcepath")
        .arg("java/netty_stubs")
        .arg("-classpath")
        .arg("java/netty_stubs")
        .arg("java/AnemoiaInterceptor.java")
        .status();

    match result {
        Ok(s) if s.success() => {
            println!("cargo:rustc-cfg=incoming_capture");
        }
        Ok(s) => {
            println!(
                "cargo:warning=javac failed (exit {:?}); incoming packet capture disabled",
                s.code()
            );
        }
        Err(e) => {
            println!(
                "cargo:warning=javac not found ({}); incoming packet capture disabled",
                e
            );
        }
    }
}
