fn main() {
    // Path to Flutter engine .so
    println!(
        "cargo:rustc-link-search=native=/home/mrhaydari/dev/flutter/flutter/bin/cache/artifacts/engine/linux-x64"
    );

    // Link against the dynamic Flutter engine library
    println!("cargo:rustc-link-lib=dylib=flutter_linux_gtk");

    // Optional: embed the rpath so Rust library finds it at runtime
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,/home/mrhaydari/dev/flutter/flutter/bin/cache/artifacts/engine/linux-x64"
    );
}
