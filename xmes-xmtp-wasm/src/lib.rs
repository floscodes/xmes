#![recursion_limit = "256"]

pub mod worker;
pub use worker::{IdentityInfo, IdentityListUpdate, XmtpHandle, init_worker_mode, is_worker_context, spawn_xmtp_worker};

use std::rc::Rc;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use anyhow::{Error, Result};
use k256::ecdsa::SigningKey;
use bindings_wasm::client::{create_client, Client, DeviceSyncMode};
pub use bindings_wasm::conversation::Conversation;
pub use bindings_wasm::conversations::Conversations;

fn short_inbox(s: &str) -> String {
    if s.len() <= 13 { s.to_string() }
    else { format!("{}…{}", &s[..6], &s[s.len()-4..]) }
}

#[derive(Clone, PartialEq)]
pub struct MemberInfo {
    pub inbox_id: String,
    pub address:  String,
    pub role:     u8, // 0=Member, 1=Admin, 2=SuperAdmin
}

#[derive(Clone, PartialEq)]
pub struct MessageInfo {
    pub id:               String,
    pub text:             String,
    pub system_text:      Option<String>, // join/leave notification; None for regular messages
    pub sender_inbox_id:  String,
    pub sent_at_ns:       i64,
    pub delivered:        bool,
}

#[derive(Clone, PartialEq)]
pub struct ConversationSummary {
    pub id: String,
    pub name: String,
    pub last_sender: Option<String>,
    pub last_message_ns: Option<i64>,
    pub is_pending: bool,
}
use bindings_wasm::conversations::{
    ListConversationsOptions,
    ListConversationsOrderBy
};
use bindings_wasm::consent_state::ConsentState as XmtpConsentState;
use bindings_wasm::messages::DeliveryStatus;
use bindings_wasm::enriched_message::DecodedMessage;
use bindings_wasm::content_types::decoded_message_content::DecodedMessageContent;
use bindings_wasm::identity::{Identifier, IdentifierKind};
use bindings_wasm::inbox_id::generate_inbox_id;

const DEFAULT_DEV_ENV_HOST: &str = "https://api.dev.xmtp.network:5558";
const DEFAULT_PRODUCTION_ENV_HOST: &str = "https://api.production.xmtp.network:5558";


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

    /// Returns all Ethereum addresses linked to this inbox on the XMTP network.
    pub async fn linked_addresses(&self) -> Vec<String> {
        match self.client.inbox_state_from_inbox_ids(vec![self.inbox_id.clone()], false).await {
            Ok(states) => states
                .into_iter()
                .flat_map(|s| s.account_identifiers)
                .map(|id| id.identifier)
                .collect(),
            Err(_) => vec![self.address.clone()],
        }
    }

    /// Link a new Ethereum address to this inbox.
    ///
    /// NOTE: Not yet implemented — the `apply_signature_request` API in
    /// libxmtp currently requires a `Backend` type whose internal fields
    /// (`bundle`, `inner`) are `pub(crate)` and are not accessible from
    /// outside the `bindings_wasm` crate.  A PR upstream or a fork is
    /// needed before this can be wired up.  The worker surfaces a
    /// user-visible error message in the meantime.
    pub async fn link_new_address(&self) -> Result<String> {
        Err(Error::msg("Address linking not yet supported — requires upstream libxmtp API change"))
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
        // Deny consent first so the group is excluded from future list() queries
        // even if the MLS leave fails (e.g. super-admin restriction).
        let _ = conversation.update_consent_state(XmtpConsentState::Denied);
        let _ = conversation.leave_group().await;
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
    pub async fn add_members_to_conversation(&self, conversation_id: String, ids: Vec<String>) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;

        let mut inbox_ids = Vec::new();
        let mut identifiers = Vec::new();
        for id in ids {
            if id.starts_with("0x") || id.starts_with("0X") {
                identifiers.push(Identifier { identifier: id.to_lowercase(), identifier_kind: IdentifierKind::Ethereum });
            } else {
                inbox_ids.push(id);
            }
        }
        if !inbox_ids.is_empty() {
            convo.add_members(inbox_ids).await.map_err(|e| Error::msg(format!("{e:?}")))?;
        }
        if !identifiers.is_empty() {
            convo.add_members_by_identity(identifiers).await.map_err(|e| Error::msg(format!("{e:?}")))?;
        }
        Ok(())
    }

    pub async fn get_conversation_members(&self, conversation_id: String) -> Result<Vec<MemberInfo>> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.sync().await.map_err(|e| Error::msg(format!("{e:?}")))?;
        let raw = convo.list_members().await
            .map_err(|e| Error::msg(format!("{e:?}")))?;

        let arr = js_sys::Array::from(&raw);
        let account_ids_key = wasm_bindgen::JsValue::from_str("accountIdentifiers");
        let identifier_key  = wasm_bindgen::JsValue::from_str("identifier");
        let permission_key  = wasm_bindgen::JsValue::from_str("permissionLevel");
        let inbox_id_key    = wasm_bindgen::JsValue::from_str("inboxId");

        let mut members = Vec::new();
        for i in 0..arr.length() {
            let member = arr.get(i);
            let Ok(id_arr_val) = js_sys::Reflect::get(&member, &account_ids_key) else { continue };
            let id_arr = js_sys::Array::from(&id_arr_val);
            let role = js_sys::Reflect::get(&member, &permission_key)
                .ok().and_then(|v| v.as_f64()).map(|f| f as u8).unwrap_or(0);
            let inbox_id = js_sys::Reflect::get(&member, &inbox_id_key)
                .ok().and_then(|v| v.as_string()).unwrap_or_default();
            for j in 0..id_arr.length() {
                let id_item = id_arr.get(j);
                if let Ok(addr_val) = js_sys::Reflect::get(&id_item, &identifier_key) {
                    if let Some(addr) = addr_val.as_string() {
                        members.push(MemberInfo { inbox_id: inbox_id.clone(), address: addr, role });
                        break;
                    }
                }
            }
        }
        Ok(members)
    }

    pub async fn remove_member(&self, conversation_id: String, inbox_id: String) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.remove_members(vec![inbox_id]).await
            .map_err(|e| Error::msg(format!("{e:?}")))
    }

    pub async fn set_admin(&self, conversation_id: String, inbox_id: String, add: bool) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        if add {
            convo.add_admin(inbox_id).await.map_err(|e| Error::msg(format!("{e:?}")))
        } else {
            convo.remove_admin(inbox_id).await.map_err(|e| Error::msg(format!("{e:?}")))
        }
    }

    pub async fn set_super_admin(&self, conversation_id: String, inbox_id: String, add: bool) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        if add {
            convo.add_super_admin(inbox_id).await.map_err(|e| Error::msg(format!("{e:?}")))
        } else {
            convo.remove_super_admin(inbox_id).await.map_err(|e| Error::msg(format!("{e:?}")))
        }
    }

    pub async fn update_group_name(&self, conversation_id: String, name: String) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.update_group_name(name).await
            .map_err(|e| Error::msg(format!("{e:?}")))
    }

    pub async fn fetch_messages(&self, conversation_id: String) -> Result<Vec<MessageInfo>> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.sync().await.map_err(|e| Error::msg(format!("{e:?}")))?;
        let msgs: Vec<DecodedMessage> = convo.find_enriched_messages(None).await
            .map_err(|e| Error::msg(format!("{e:?}")))?;
        let result = msgs.into_iter().filter_map(|m| {
            let delivered = matches!(m.delivery_status, DeliveryStatus::Published);
            match m.content {
                DecodedMessageContent::Text { content } => {
                    if content.is_empty() { return None; }
                    Some(MessageInfo {
                        id: m.id, text: content, system_text: None,
                        sender_inbox_id: m.sender_inbox_id,
                        sent_at_ns: m.sent_at_ns, delivered,
                    })
                }
                DecodedMessageContent::GroupUpdated { content } => {
                    if !content.left_inboxes.is_empty() {
                        let label = short_inbox(&content.left_inboxes[0].inbox_id);
                        Some(MessageInfo {
                            id: m.id, text: String::new(),
                            system_text: Some(format!("{label} left the group")),
                            sender_inbox_id: m.sender_inbox_id,
                            sent_at_ns: m.sent_at_ns, delivered: true,
                        })
                    } else if !content.added_inboxes.is_empty() {
                        // Only show notification for voluntary joins (initiator == joined person)
                        let voluntary = content.added_inboxes.iter()
                            .any(|i| i.inbox_id == content.initiated_by_inbox_id);
                        if voluntary {
                            let label = short_inbox(&content.added_inboxes[0].inbox_id);
                            Some(MessageInfo {
                                id: m.id, text: String::new(),
                                system_text: Some(format!("{label} joined the group")),
                                sender_inbox_id: m.sender_inbox_id,
                                sent_at_ns: m.sent_at_ns, delivered: true,
                            })
                        } else { None }
                    } else { None }
                }
                _ => None,
            }
        }).collect();
        Ok(result)
    }

    pub async fn send_text_message(&self, conversation_id: String, text: String) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.send_text(text, None).await
            .map_err(|e| Error::msg(format!("{e:?}")))?;
        Ok(())
    }

    pub fn accept_invitation(&self, conversation_id: String) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.update_consent_state(XmtpConsentState::Allowed)
            .map_err(|e| Error::msg(format!("{e:?}")))?;
        Ok(())
    }

    pub fn decline_invitation(&self, conversation_id: String) -> Result<()> {
        let convo = self.conversations()
            .find_group_by_id(conversation_id)
            .map_err(|_| Error::msg("Conversation not found"))?;
        convo.update_consent_state(XmtpConsentState::Denied)
            .map_err(|e| Error::msg(format!("{e:?}")))?;
        Ok(())
    }

    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.conversations()
            .sync_all_conversations(None)
            .await
            .map_err(|e| {
                web_sys::console::error_1(&format!("sync_all_conversations error: {:?}", e).into());
                Error::msg("Could not sync conversations")
            })?;

        let convos_array = self.conversations()
            .list(Some(
                ListConversationsOptions {
                    order_by: Some(ListConversationsOrderBy::LastActivity),
                    ..Default::default()
                }
            ))
            .map_err(|_| Error::msg("Could not list conversations"))?;

        let convo_key            = wasm_bindgen::JsValue::from_str("conversation");
        let id_key               = wasm_bindgen::JsValue::from_str("id");
        let group_name_key       = wasm_bindgen::JsValue::from_str("groupName");
        let last_msg_key         = wasm_bindgen::JsValue::from_str("lastMessage");
        let sender_key           = wasm_bindgen::JsValue::from_str("senderInboxId");
        let sent_at_ns_key       = wasm_bindgen::JsValue::from_str("sentAtNs");
        let membership_state_key = wasm_bindgen::JsValue::from_str("membershipState");
        let consent_state_key    = wasm_bindgen::JsValue::from_str("consentState");

        // Pre-fetch the global String() function for BigInt-to-string conversion.
        let js_string_fn: Option<js_sys::Function> = js_sys::Reflect::get(
            &js_sys::global(),
            &wasm_bindgen::JsValue::from_str("String"),
        ).ok().map(js_sys::Function::from);

        struct RawItem {
            id: String,
            name: String,
            sender_inbox_id: Option<String>,
            last_message_ns: Option<i64>,
            is_pending: bool,
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

            let last_msg_val = js_sys::Reflect::get(&item, &last_msg_key)
                .ok()
                .filter(|v| !v.is_null() && !v.is_undefined());

            let sender_inbox_id = last_msg_val.as_ref()
                .and_then(|last_msg| js_sys::Reflect::get(last_msg, &sender_key).ok())
                .and_then(|v| v.as_string());

            let last_message_ns = last_msg_val.as_ref()
                .and_then(|last_msg| js_sys::Reflect::get(last_msg, &sent_at_ns_key).ok())
                .and_then(|v| {
                    // sentAtNs is a JS BigInt — as_f64() returns None for BigInt.
                    // Use the global String() function which handles BigInt reliably.
                    if let Some(f) = v.as_f64() {
                        return if f > 0.0 { Some(f as i64) } else { None };
                    }
                    let s = js_string_fn.as_ref()
                        .and_then(|f| f.call1(&wasm_bindgen::JsValue::NULL, &v).ok())
                        .and_then(|v| v.as_string())?;
                    let n = s.parse::<i64>().ok()?;
                    if n > 0 { Some(n) } else { None }
                });

            // Call membershipState() and consentState() directly on the convo object from list().
            // GroupMembershipState: Allowed=0, Rejected=1, Pending=2, Restored=3, PendingRemove=4
            // ConsentState:         Unknown=0, Allowed=1, Denied=2
            let membership_state_val = js_sys::Reflect::get(&convo, &membership_state_key)
                .ok()
                .and_then(|f| js_sys::Function::from(f).call0(&convo).ok())
                .and_then(|v| v.as_f64())
                .map(|f| f as u32);

            let Some(ms) = membership_state_val else { continue };

            if ms == 1 || ms == 4 { continue; } // Rejected or PendingRemove

            // consentState() can fail for a newly-received conversation; treat failure as Unknown(0).
            let cs: u32 = js_sys::Reflect::get(&convo, &consent_state_key)
                .ok()
                .and_then(|f| js_sys::Function::from(f).call0(&convo).ok())
                .and_then(|v| v.as_f64())
                .map(|f| f as u32)
                .unwrap_or(0);

            // Pending membership + consent not yet set = unanswered invitation.
            // If the user already accepted (cs=Allowed=1), show as a normal conversation
            // even if membership hasn't been upgraded yet by the next sync.
            let is_pending = ms == 2 && cs != 1;

            raw_items.push(RawItem { id, name, sender_inbox_id, last_message_ns, is_pending });
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
            ConversationSummary { id: it.id, name: it.name, last_sender, last_message_ns: it.last_message_ns, is_pending: it.is_pending }
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
