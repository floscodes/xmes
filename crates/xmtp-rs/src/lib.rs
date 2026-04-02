use anyhow::{Error, Result};
use bindings_wasm::client::{Client, create_client};
use bindings_wasm::conversation::{self, Conversation};
use bindings_wasm::identity::{Identifier, IdentifierKind};
use bindings_wasm::inbox_id::generate_inbox_id;

mod wallet;
use wallet::LocalWallet;

#[cfg(test)]
mod tests;

pub struct Profile {
    address: String,
    inbox_id: String,
    env: Env,
    keypair: Keypair,
    client: Client,
}

pub(crate) struct Keypair {
    private: String,
    public: String,
}

#[derive(Default)]
pub enum Env {
    Local(String),
    #[default]
    Dev,
    Production,
}

impl Env {
    pub fn get_host(&self) -> String {
        match self {
            Env::Local(localhost) => localhost.to_owned(),
            Env::Dev => "https://grpc.dev.xmtp.network:443".to_string(),
            Env::Production => "https://grpc.production.xmtp.network:443".to_string(),
        }
    }
}

pub async fn create_profile() -> Result<Profile> {
    let wallet = LocalWallet::random();
    let identifier = Identifier {
        identifier: wallet.address.clone(),
        identifier_kind: IdentifierKind::Ethereum,
    };
    let inbox_id = generate_inbox_id(identifier.clone(), None)
        .map_err(|_| Error::msg("Could not generate inbox id"))?;
    let mut client = create_client(
        "https://grpc.dev.xmtp.network:443".to_string(),
        inbox_id.clone(),
        identifier,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .map_err(|_| Error::msg("Could not create client"))?;

    if !client.is_registered() {
        let sig_request = client
            .create_inbox_signature_request()
            .map_err(|_| Error::msg("Could not create signature request"))?
            .unwrap();

        let text = sig_request.signature_text().await
            .map_err(|_| Error::msg("Signing failed!"))?;

        let sig_bytes = wallet.sign(&text);
        let sig_uint8 = js_sys::Uint8Array::from(sig_bytes.as_slice());

        sig_request.add_ecdsa_signature(sig_uint8).await
            .map_err(|_| Error::msg("Could not add signature"))?;
        client.register_identity(sig_request).await
            .map_err(|_| Error::msg("Could not register identity"))?;
    }

    Ok(
        Profile {
            address: wallet.address.clone(),
            inbox_id: inbox_id,
            env: Env::default(),
            keypair: wallet.keypair(),
            client: client,
        }
    )
}
