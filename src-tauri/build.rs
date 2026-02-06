fn main() {
    #[cfg(target_os = "macos")]
    build_hotkey_helper();

    // XOR key for obfuscation (16 bytes)
    const XOR_KEY: [u8; 16] = [
        0x4f, 0x72, 0x61, 0x6e, 0x67, 0x65, 0x50, 0x69, 0x6e, 0x65, 0x61, 0x70, 0x70, 0x6c, 0x65,
        0x21,
    ];

    // Check if OPENROUTER_API_KEY environment variable is set at build time
    if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
        if !api_key.is_empty() {
            // XOR obfuscate the key
            let obfuscated: Vec<u8> = api_key
                .bytes()
                .enumerate()
                .map(|(i, b)| b ^ XOR_KEY[i % XOR_KEY.len()])
                .collect();

            // Hex-encode the obfuscated bytes
            let hex_encoded: String = obfuscated.iter().map(|b| format!("{:02x}", b)).collect();

            // Pass to rustc as environment variables
            println!("cargo:rustc-env=OBFUSCATED_API_KEY={}", hex_encoded);
            println!("cargo:rustc-env=HAS_BUILTIN_KEY=1");
        } else {
            println!("cargo:rustc-env=OBFUSCATED_API_KEY=");
            println!("cargo:rustc-env=HAS_BUILTIN_KEY=0");
        }
    } else {
        println!("cargo:rustc-env=OBFUSCATED_API_KEY=");
        println!("cargo:rustc-env=HAS_BUILTIN_KEY=0");
    }

    // Re-run build script if OPENROUTER_API_KEY changes
    println!("cargo:rerun-if-env-changed=OPENROUTER_API_KEY");

    tauri_build::build()
}

#[cfg(target_os = "macos")]
fn build_hotkey_helper() {
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let helper_src = manifest_dir.join("hotkey-helper/main.swift");
    let resources_dir = manifest_dir.join("resources");
    let helper_out = resources_dir.join("oto-hotkey-helper");

    println!("cargo:rerun-if-changed={}", helper_src.display());

    if !helper_src.exists() {
        eprintln!(
            "[build] Hotkey helper source not found at {}",
            helper_src.display()
        );
        return;
    }

    if let Err(e) = std::fs::create_dir_all(&resources_dir) {
        panic!(
            "Failed to create resources dir {}: {}",
            resources_dir.display(),
            e
        );
    }

    let module_cache_dir = manifest_dir.join("target/swift-module-cache");
    if let Err(e) = std::fs::create_dir_all(&module_cache_dir) {
        panic!(
            "Failed to create Swift module cache dir {}: {}",
            module_cache_dir.display(),
            e
        );
    }

    let status = Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "swiftc",
            helper_src
                .to_str()
                .expect("helper_src contains non-utf8 path"),
            "-O",
            "-module-cache-path",
            module_cache_dir
                .to_str()
                .expect("module_cache_dir contains non-utf8 path"),
            "-framework",
            "ApplicationServices",
            "-o",
            helper_out
                .to_str()
                .expect("helper_out contains non-utf8 path"),
        ])
        .status()
        .expect("Failed to run xcrun swiftc");

    if !status.success() {
        panic!("Failed to compile hotkey helper (swiftc exit code: {status})");
    }
}
