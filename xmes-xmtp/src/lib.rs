use alloy::signers::Signer;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{Error, Result};
use bindings_wasm::client::{Client, create_client};
use bindings_wasm::conversation::{self, Conversation};
use bindings_wasm::identity::{Identifier, IdentifierKind};
use bindings_wasm::inbox_id::generate_inbox_id;
use std::fs;
use toml::Table;

const DEFAULT_DEV_ENV_HOST: &'static str = "https://api.dev.xmtp.network:5558";
const DEFAULT_PRODUCTION_ENV_HOST: &'static str = "https://api.production.xmtp.network:5558";

pub struct Identity {
    inbox_id: String,
    addresses: Vec<String>,
    env: Env,
    client: Client,
}

impl Identity {
    pub async fn new(env: Env) -> Result<Identity> {
        let signer = PrivateKeySigner::random();
        let identifier = Identifier {
            identifier: signer.address().to_string(),
            identifier_kind: IdentifierKind::Ethereum,
        };

        let inbox_id = generate_inbox_id(identifier.clone(), None)
            .map_err(|_| Error::msg("Could not generate inbox id"))?;
        let mut client: Client = create_client(
            env.host(),
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
            let sig_uint8 = js_sys::Uint8Array::from(signature.as_bytes().as_slice());

            sig_request
                .add_ecdsa_signature(sig_uint8)
                .await
                .map_err(|_| Error::msg("Could not add signature"))?;
            client
                .register_identity(sig_request)
                .await
                .map_err(|_| Error::msg("Could not register identity"))?;
        }

        Ok(Identity {
            addresses: vec![signer.address().to_string()],
            inbox_id,
            env,
            client,
        })
    }
    pub async fn from_toml(file_path: &str) -> Result<Vec<Self>> {
        let toml_file = fs::read_to_string(file_path)
            .map_err(|_| Error::msg("Failed to open TOML file."))?
            .parse::<Table>()
            .map_err(|e| Error::msg(format!("Failed to parse TOML: {}", e)))?;
        let Identitys = toml_file["Identitys"].as_array().ok_or(Error::msg(
            "Failed to parse Identitys - are there any Identitys set?",
        ))?;

        let mut Identity_vec = Vec::new();

        for Identity in Identitys {
            let address = Identity["address"]
                .as_str()
                .ok_or(Error::msg("Failed to parse Identity"))?;
            let inbox_id = Identity["inbox_id"]
                .as_str()
                .ok_or(Error::msg("Failed to parse Identity"))?;
            let env = Identity["env"]
                .as_table()
                .ok_or(Error::msg("Failed to parse Identity"))?;
            let env_name = env["environment"].as_str().unwrap_or_default();
            let host = env["host"].as_str().unwrap_or_default();
            let environment = match env_name {
                "local" => Env::Local(host.to_string()),
                "dev" => Env::Dev(Some(host.to_string())),
                "production" => Env::Production(Some(host.to_string())),
                _ => Env::default(),
            };
            let identifier = Identifier {
                identifier: address.to_string(),
                identifier_kind: IdentifierKind::Ethereum,
            };
            let client: Client = create_client(
                environment.host().to_string(),
                inbox_id.to_string(),
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
            .map_err(|_| Error::msg("Failed to create client"))?;

            Identity_vec.push(Identity {
                addresses: vec![address.to_string()],
                inbox_id: inbox_id.to_string(),
                env: environment,
                client,
            });
        }

        Ok(Identity_vec)
    }

    pub fn to_toml(&self) -> String {
        let mut addresses_str = String::from("addresses = [");
        for address in &self.addresses {
            addresses_str.push_str(&format!("\"{}\",", address));
        }
        addresses_str.push_str("]");
        format!(
            r#"
            [[identity]]
            inbox_id = "{}"
            {}
            [env]
            environment = "{}"
            host = "{}"
            "#,
            self.inbox_id, addresses_str, self.env.name(), self.env.host()
        )
    }

    pub fn addresses(&self) -> Vec<String> {
        self.addresses.clone()
    }
    pub fn client(&self) -> &Client {
        &self.client
    }
    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }
    pub fn inbox_id(&self) -> String {
        self.inbox_id.clone()
    }
    pub fn env(&self) -> &Env {
        &self.env
    }
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }
}

pub type EnvHost = String;

pub enum Env {
    Local(EnvHost),
    Dev(Option<EnvHost>),
    Production(Option<EnvHost>),
}

impl Default for Env {
    fn default() -> Self {
        Self::Dev(Some(DEFAULT_DEV_ENV_HOST.to_string()))
    }
}

impl Env {
    fn name(&self) -> &'static str {
        match self {
            Self::Local(_) => "local",
            Self::Dev(_) => "dev",
            Self::Production(_) => "production",
        }
    }

    fn host(&self) -> String {
        match self {
            Self::Local(host) => {
                if !host.contains(":") {
                    return format!("http://{}:5558", host);
                }
                host.to_owned()
            }
            Self::Dev(host) => if let Some(host) = host {
                host.clone()
            } else {
                DEFAULT_DEV_ENV_HOST.to_string()
            },
            Self::Production(host) => if let Some(host) = host {
                host.clone()
            } else {
                DEFAULT_PRODUCTION_ENV_HOST.to_string()
            },
        }
    }
}
