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

pub struct Profile {
    address: String,
    inbox_id: String,
    env: Env,
    pub client: Client,
}

impl Profile {
    pub async fn new(env: Env) -> Result<Profile> {
        let signer = PrivateKeySigner::random();
        let identifier = Identifier {
            identifier: signer.address().to_string(),
            identifier_kind: IdentifierKind::Ethereum,
        };

        let inbox_id = generate_inbox_id(identifier.clone(), None)
            .map_err(|_| Error::msg("Could not generate inbox id"))?;
        let mut client: Client = create_client(
            env.0.to_string(),
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
    pub async fn from_toml(file_path: &str) -> Result<Vec<Self>> {
        let toml_file = fs::read_to_string(file_path)
            .map_err(|_| Error::msg("Failed to open TOML file."))?
            .parse::<Table>()
            .map_err(|e| Error::msg(format!("Failed to parse TOML: {}", e)))?;
        let profiles = toml_file["profiles"].as_array().ok_or(Error::msg(
            "Failed to parse profiles - are there any profiles set?",
        ))?;

        let mut profile_vec = Vec::new();

        for profile in profiles {
            let address = profile["address"]
                .as_str()
                .ok_or(Error::msg("Failed to parse profile"))?;
            let inbox_id = profile["inbox_id"]
                .as_str()
                .ok_or(Error::msg("Failed to parse profile"))?;
            let env = profile["env"]
                .as_table()
                .ok_or(Error::msg("Failed to parse profile"))?;
            let env_name = env["environment"].as_str().ok_or_default();
            let host = env["host"].as_str().ok_or_default();
            let environment = match env_name {
                "local" => Env::Local(host),
                "dev" => Env::Dev(host),
                "production" => Env::Production(host),
                _ => Env::default(),
            };
            let identifier = Identifier {
                identifier: address.to_string(),
                identifier_kind: IdentifierKind::Ethereum,
            };
            let client: Client = create_client(
                environment.get_host().to_string(),
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
            .map_err(|e| Error::msg(format!("Failed to create client: {}", e)))?;

            profile_vec.push(Profile {
                address: address.to_string(),
                inbox_id: inbox_id.to_string(),
                env: environment,
                client,
            });
        }

        Ok(profile_vec)
    }

    pub async fn save_to_file(&self, path: &str) -> Result<()> {
        let mut toml_file: Option<Table> = fs::read_to_string(path)
            .ok_or(None)
            .parse::<Table>()
            .map_err(|e| Error::msg(format!("Failed to parse TOML: {}", e)))?;

        let new_config = format!(
            r#"
            [[profiles]]
            address = "{}"
            inbox_id = "{}"
            [env]
            environment = "{}"
            host = "{}"
            "#,
            self.address,
            self.inbox_id,
            self.env.get_name(),
            self.env.get_host()
        );

        if toml_file.is_none() {
            fs::write(path, new_config)
                .map_err(|e| Error::msg(format!("Failed to write to file: {}", e)))?;
            return Ok(());
        }

        let config = format!("{}\n\n{}", toml_file.unwrap(), new_config);
        fs::write(path, config)
            .map_err(|e| Error::msg(format!("Failed to write to file: {}", e)))?;

        Ok(())
    }
}

pub type EnvHost = String;

#[derive(Default)]
pub enum Env {
    Local(EnvHost),
    #[default]
    Dev(Some(EnvHost)),
    Production(Some(EnvHost)),
}

impl Env {
    fn get_name() -> &'static str {
        match self {
            Self::Local(_) => "local",
            Self::Dev(_) => "dev",
            Self::Production(_) => "production",
        }
    }

    fn get_host(&self) -> String {
        match self {
            Self::Local(host) => {
                if !host.contains(":") {
                    return format!("http://{}:5558", localhost);
                }
                host.to_owned()
            }
            Self::Dev(host) => host.unwrap_or(DEFAULT_DEV_ENV_HOST.to_string()),
            Self::Production(host) => host.unwrap_or(DEFAULT_PRODUCTION_ENV_HOST.to_string()),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::Dev(Some(DEFAULT_DEV_ENV_HOST.to_string()))
    }
}
