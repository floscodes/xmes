use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use anyhow::{Error, Result};
use bindings_wasm::client::{create_client, Client};
use bindings_wasm::conversation::{self, Conversation};
use bindings_wasm::identity::{Identifier, IdentifierKind};
use bindings_wasm::inbox_id::generate_inbox_id;

pub struct Profile {
    address: String,
    inbox_id: String,
    env: Env,
    pub client: Client,
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
            Env::Local(localhost) => {
                if !localhost.contains(":") {
                    return format!("http://{}:5558", localhost);
                }
                localhost.to_owned()
            }
            Env::Dev => "https://api.dev.xmtp.network:5558".to_string(),
            Env::Production => "https://api.production.xmtp.network:5558".to_string(),
        }
    }
}

pub async fn create_profile(env: Env) -> Result<Profile> {
    let signer = PrivateKeySigner::random();
    let identifier = Identifier {
        identifier: signer.address().to_string(),
        identifier_kind: IdentifierKind::Ethereum,
    };

    let inbox_id = generate_inbox_id(identifier.clone(), None)
        .map_err(|_| Error::msg("Could not generate inbox id"))?;
    let mut client: Client = create_client(
        env.get_host().to_string(),
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

        let text = sig_request
            .signature_text()
            .await
            .map_err(|_| Error::msg("Signing failed!"))?;

        let signature = signer
            .sign_message(&text.as_bytes())
            .await
            .map_err(|_| Error::msg("Could not sign message"))?;
        let sig_uint8 = js_sys::Uint8Array::from(signature.to_string().as_bytes());

        sig_request
            .add_ecdsa_signature(sig_uint8)
            .await
            .map_err(|_| Error::msg("Could not add signature"))?;
        client
            .register_identity(sig_request)
            .await
            .map_err(|_| Error::msg("Could not register identity"))?;
    }

    Ok(Profile {
        address: signer.address().to_string(),
        inbox_id,
        env,
        client,
    })
}
