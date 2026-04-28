curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

source "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown

cargo binstall dioxus-cli --force

curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

cargo binstall 

curl -L https://github.com/llvm/llvm-project/releases/download/llvmorg-17.0.6/clang+llvm-17.0.6-x86_64-linux-gnu-ubuntu-22.04.tar.xz | tar -xJ
export PATH="$PWD/clang+llvm-17.0.6-x86_64-linux-gnu-ubuntu-22.04/bin:$PATH"
export CC="$PWD/clang+llvm-17.0.6-x86_64-linux-gnu-ubuntu-22.04/bin/clang"
export CC_wasm32_unknown_unknown="$PWD/clang+llvm-17.0.6-x86_64-linux-gnu-ubuntu-22.04/bin/clang"

export PRODUCTION=1
export PUSH_WORKER_URL="https://push-worker.xmes.org"
dx build --release --web

mkdir -p target/dx/xmes-mobile-pwa/release/web
mv /opt/buildhome/repo/target/dx/xmes-mobile-pwa/release/web/public target/dx/xmes-mobile-pwa/release/web/public