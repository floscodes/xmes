curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

source "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown

cargo install dioxus-cli

which clang clang-14 clang-15 clang-16 clang-17 clang-18 2>/dev/null || echo "no clang found"
ls /usr/bin/clang* 2>/dev/null || echo "no clang in /usr/bin"

sudo apt install -y clang libzstd-dev

dx build --release --web