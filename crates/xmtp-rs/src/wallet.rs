use super::Keypair;
use k256::ecdsa::{SigningKey, RecoveryId, Signature};
use k256::elliptic_curve::rand_core::OsRng;
use sha3::{Digest, Keccak256};

pub(crate) struct LocalWallet {
    pub signing_key: SigningKey,
    pub address: String,
}

impl LocalWallet {
    pub(crate) fn random() -> Self {
        let signing_key = SigningKey::random(&mut OsRng);

        // Derive Ethereum address
        let verifying_key = signing_key.verifying_key();
        let public_key = verifying_key.to_encoded_point(false);
        let public_key_bytes = &public_key.as_bytes()[1..]; // 0x04-Prefix entfernen
        let hash = Keccak256::digest(public_key_bytes);
        let address = format!("0x{}", hex::encode(&hash[12..]));

        Self {
            signing_key,
            address,
        }
    }

pub(crate) fn sign(&self, text: &str) -> Vec<u8> {

    let prefixed = format!("\x19Ethereum Signed Message:\n{}{}", text.len(), text);

    let (sig, recovery_id): (Signature, RecoveryId) = self
        .signing_key
        .sign_digest_recoverable(Keccak256::new_with_prefix(prefixed.as_bytes()))
        .unwrap();

    let mut bytes = sig.to_bytes().to_vec();
    bytes.push(recovery_id.to_byte());
    bytes
}

    pub(crate) fn keypair(&self) -> Keypair {
        let private = hex::encode(self.signing_key.to_bytes());

        let public = hex::encode(
            self.signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );

        Keypair { private, public }
    }
}
