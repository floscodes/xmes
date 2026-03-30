fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    if target == "wasm32-unknown-unknown" && cfg!(target_os = "macos") {
        let llvm_bin = "/opt/homebrew/opt/llvm/bin";

        println!(
            "cargo:rustc-env=CC_wasm32_unknown_unknown={}/clang",
            llvm_bin
        );
        println!(
            "cargo:rustc-env=AR_wasm32_unknown_unknown={}/llvm-ar",
            llvm_bin
        );
    }
}
