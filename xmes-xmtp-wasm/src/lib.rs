#![recursion_limit = "256"]

pub mod worker;
pub use worker::{XmtpHandle, init_worker_mode, is_worker_context, spawn_xmtp_worker};

use std::rc::Rc;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use anyhow::{Error, Result};
use k256::ecdsa::SigningKey;
use bindings_wasm::client::{create_client, Client, DeviceSyncMode};
pub use bindings_wasm::conversation::Conversation;
pub use bindings_wasm::conversations::Conversations;

#[derive(Clone, PartialEq)]
pub struct ConversationSummary {
    pub id: String,
    pub name: String,
    pub last_sender: Option<String>,
}
use bindings_wasm::conversations::{
    ListConversationsOptions,
    ListConversationsOrderBy
};
use bindings_wasm::identity::{Identifier, IdentifierKind};
use bindings_wasm::inbox_id::generate_inbox_id;

const DEFAULT_DEV_ENV_HOST: &'static str = "https://api.dev.xmtp.network:5558";
const DEFAULT_PRODUCTION_ENV_HOST: &'static str = "https://api.production.xmtp.network:5558";

#[derive(Clone)]
pub struct Identity {
    address: String,
    inbox_id: String,
    env: Env,
    client: Rc<Client>,
    signing_key: SigningKey,
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
            Some(inbox_id.clone()),
            None,
            Some(DeviceSyncMode::Disabled),
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
            Self::register(&mut client, &signer).await?;
        }

        Ok(Identity {
            address: signer.address().to_string().to_lowercase(),
            inbox_id,
            env,
            client: Rc::new(client),
            signing_key: signer.credential().clone(),
        })
    }
    async fn register(client: &mut Client, signer: &PrivateKeySigner) -> Result<()> {
        let sig_request = client
            .create_inbox_signature_request()
            .map_err(|_| Error::msg("Could not create signature request"))?
            .unwrap();

        let text = sig_request
            .signature_text()
            .await
            .map_err(|_| Error::msg("Signing failed!"))?;

        let signature = signer
            .sign_message(text.as_bytes())
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

        Ok(())
    }

    /// Restore an identity from a previously stored private key hex string.
    /// `address` and `inbox_id` are re-derived from the key — nothing else is stored.
    pub async fn from_key_hex(hex_str: String, env: Env) -> Result<Self> {
        let key_bytes = hex::decode(&hex_str)
            .map_err(|_| Error::msg("Invalid signing key hex"))?;
        let signing_key = SigningKey::from_bytes(k256::FieldBytes::from_slice(&key_bytes))
            .map_err(|_| Error::msg("Invalid signing key bytes"))?;

        let signer = PrivateKeySigner::from_signing_key(signing_key.clone());
        let address = signer.address().to_string().to_lowercase();

        let identifier = Identifier {
            identifier: address.clone(),
            identifier_kind: IdentifierKind::Ethereum,
        };
        let inbox_id = generate_inbox_id(identifier.clone(), None)
            .map_err(|_| Error::msg("Could not generate inbox id"))?;

        let mut client: Client = create_client(
            env.host(),
            inbox_id.clone(),
            identifier,
            Some(inbox_id.clone()),
            None,
            Some(DeviceSyncMode::Disabled),
            None, None, None, None, None, None, None, None,
        )
        .await
        .map_err(|_| Error::msg("Could not create client"))?;

        if !client.is_registered() {
            Self::register(&mut client, &signer).await?;
        }

        Ok(Identity { address, inbox_id, env, client: Rc::new(client), signing_key })
    }

    /// Serialize to a 64-character hex string (the raw private key bytes).
    pub fn to_key_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }

    pub fn signer(&self) -> PrivateKeySigner {
        PrivateKeySigner::from_signing_key(self.signing_key.clone())
    }

    pub fn address(&self) -> String {
        self.address.clone()
    }
    pub fn client(&self) -> &Client {
        &self.client
    }
    pub fn conversations(&self) -> Conversations {
        self.client.conversations()
    }
    pub async fn leave_conversation(&self, id: String) -> Result<()> {
        let conversation = self.conversations()
            .find_group_by_id(id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        conversation.leave_group()
            .await
            .map_err(|_| Error::msg("Could not leave conversation"))?;
        Ok(())
    }

    pub async fn create_group(&self) -> Result<Conversation> {
        let inbox_ids = vec![self.inbox_id().clone()];
        let convo = self.conversations()
            .create_group(inbox_ids, None)
            .await
            .map_err(|_| Error::msg("Could not create group"))?;
        Ok(convo)
    }
    pub fn env(&self) -> &Env {
        &self.env
    }
    pub fn inbox_id(&self) -> String {
        self.inbox_id.clone()
    }
    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.conversations()
            .sync_all_conversations(None)
            .await
            .map_err(|_| Error::msg("Could not sync conversations"))?;

        let convos_array = self.conversations()
            .list(Some(
                ListConversationsOptions {
                    order_by: Some(ListConversationsOrderBy::LastActivity),
                    ..Default::default()
                }
            ))
            .map_err(|_| Error::msg("Could not list conversations"))?;

        let convo_key      = wasm_bindgen::JsValue::from_str("conversation");
        let id_key         = wasm_bindgen::JsValue::from_str("id");
        let group_name_key = wasm_bindgen::JsValue::from_str("groupName");
        let last_msg_key   = wasm_bindgen::JsValue::from_str("lastMessage");
        let sender_key     = wasm_bindgen::JsValue::from_str("senderInboxId");

        struct RawItem {
            id: String,
            name: String,
            sender_inbox_id: Option<String>,
        }

        // Sync pass: extract conversation data + sender inbox IDs
        let mut raw_items: Vec<RawItem> = Vec::new();
        for i in 0..convos_array.length() {
            let item = convos_array.get(i);
            let Ok(convo) = js_sys::Reflect::get(&item, &convo_key) else { continue };

            let Ok(id_fn_val) = js_sys::Reflect::get(&convo, &id_key) else { continue };
            let Ok(id_val) = js_sys::Function::from(id_fn_val).call0(&convo) else { continue };
            let Some(id) = id_val.as_string() else { continue };

            let name = js_sys::Reflect::get(&convo, &group_name_key)
                .ok()
                .and_then(|fn_val| js_sys::Function::from(fn_val).call0(&convo).ok())
                .and_then(|v| v.as_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| id[..8.min(id.len())].to_string());

            let sender_inbox_id = js_sys::Reflect::get(&item, &last_msg_key)
                .ok()
                .filter(|v| !v.is_null() && !v.is_undefined())
                .and_then(|last_msg| js_sys::Reflect::get(&last_msg, &sender_key).ok())
                .and_then(|v| v.as_string());

            raw_items.push(RawItem { id, name, sender_inbox_id });
        }

        // Async pass: batch-resolve sender inbox IDs → wallet addresses
        let sender_ids: Vec<String> = raw_items.iter()
            .filter_map(|it| it.sender_inbox_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut addr_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if !sender_ids.is_empty() {
            if let Ok(states) = self.client.inbox_state_from_inbox_ids(sender_ids, false).await {
                for state in states {
                    if let Some(addr) = state.account_identifiers.into_iter().next() {
                        addr_map.insert(state.inbox_id, addr.identifier);
                    }
                }
            }
        }

        let summaries = raw_items.into_iter().map(|it| {
            let last_sender = it.sender_inbox_id
                .as_deref()
                .and_then(|inbox_id| addr_map.get(inbox_id))
                .cloned();
            ConversationSummary { id: it.id, name: it.name, last_sender }
        }).collect();

        Ok(summaries)
    }
}

pub type EnvHost = String;

#[derive(Clone)]
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
            Self::Dev(host) => {
                if let Some(host) = host {
                    host.clone()
                } else {
                    DEFAULT_DEV_ENV_HOST.to_string()
                }
            }
            Self::Production(host) => {
                if let Some(host) = host {
                    host.clone()
                } else {
                    DEFAULT_PRODUCTION_ENV_HOST.to_string()
                }
            }
        }
    }
}
