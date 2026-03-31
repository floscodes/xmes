use bindings_wasm::client::XmtpEnv;
use bindings_wasm::client::backend::{Backend, BackendBuilder};

pub fn generate_identity() {
    let env = XmtpEnv::Production;
    let backend = BackendBuilder::new(env).build().unwrap();
}