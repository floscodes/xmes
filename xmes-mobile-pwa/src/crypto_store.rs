use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};

const WRAPPING_KEY_STORAGE: &str = "xmes-wrapping-key";

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn get_or_create_key() -> Option<[u8; 32]> {
    let storage = local_storage()?;

    if let Some(b64) = storage.get_item(WRAPPING_KEY_STORAGE).ok().flatten() {
        if let Ok(bytes) = B64.decode(b64) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Some(key);
            }
        }
    }

    // Generate and persist a new wrapping key.
    let key = Aes256Gcm::generate_key(OsRng);
    let _ = storage.set_item(WRAPPING_KEY_STORAGE, &B64.encode(key.as_slice()));
    Some(key.into())
}

/// Encrypts `plaintext` with AES-GCM-256 and returns `iv || ciphertext` as base64.
pub fn encrypt(plaintext: &str) -> Option<String> {
    let key_bytes = get_or_create_key()?;
    let cipher    = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce     = Aes256Gcm::generate_nonce(OsRng);
    let ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes()).ok()?;

    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Some(B64.encode(&combined))
}

/// Decrypts a value produced by `encrypt`. Returns `None` on failure,
/// which signals a migration case (value is still plaintext).
pub fn decrypt(b64: &str) -> Option<String> {
    let combined = B64.decode(b64).ok()?;
    if combined.len() < 12 { return None; }

    let key_bytes = get_or_create_key()?;
    let cipher    = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce     = Nonce::from_slice(&combined[..12]);
    let plaintext = cipher.decrypt(nonce, &combined[12..]).ok()?;
    String::from_utf8(plaintext).ok()
}
