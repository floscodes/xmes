curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

source "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown

cargo install dioxus-cli

sudo apt install -y clang libzstd-dev

dx build --release --web