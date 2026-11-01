//! Encryption at rest for what the database must not hold in the clear: an event's PII and
//! the notes rendered from it.
//!
//! XChaCha20-Poly1305 with a random 24-byte nonce per seal, so there is no nonce
//! bookkeeping to get wrong (the same construction as banking's signer `key_vault`). What a
//! blob belongs to — the event id, the note's rule and event — is bound as associated data, so a blob
//! copied onto another row does not open there.
//!
//! This protects a stolen dump or backup. It does not protect against the running
//! process, which holds the key.

// `chacha20poly1305 0.10` re-exports `generic-array 0.14`, whose `from_slice` is deprecated
// in favour of an API the crate has not moved to; the calls are right for this version.
#![allow(deprecated)]

use chacha20poly1305::{
	Key, XChaCha20Poly1305, XNonce,
	aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const NONCE_LEN: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SealError {
	#[error("PANEL_DATA_KEY must be 64 hex characters (32 bytes)")]
	KeyFormat,
	#[error("a sealed blob does not open: wrong data key, or it was tampered with or moved")]
	Open,
	#[error("encryption failed")]
	Seal,
	#[error("the OS random source failed")]
	Random,
}

/// The data key, from the environment. Wiped on drop.
pub struct DataKey {
	key: Zeroizing<[u8; 32]>,
}

impl DataKey {
	pub fn from_hex(hex_key: &str) -> Result<Self, SealError> {
		let raw = Zeroizing::new(hex::decode(hex_key.trim()).map_err(|_| SealError::KeyFormat)?);
		let key: [u8; 32] = raw.as_slice().try_into().map_err(|_| SealError::KeyFormat)?;
		Ok(Self { key: Zeroizing::new(key) })
	}

	/// A fresh random key, as `sites_panel gen-data-key` prints it.
	pub fn generate_hex() -> Result<String, SealError> {
		let mut key = Zeroizing::new([0u8; 32]);
		getrandom::fill(key.as_mut_slice()).map_err(|_| SealError::Random)?;
		Ok(hex::encode(key.as_slice()))
	}

	/// Names the key without revealing it: stored beside every blob, so a blob sealed under
	/// another key is known for one before anything tries to open it.
	pub fn fingerprint(&self) -> [u8; 32] {
		let mut h = Sha256::new();
		h.update(b"sa-panel/data-key-fp/v1");
		h.update(self.key.as_slice());
		h.finalize().into()
	}

	/// A MAC of an event's canonical form, under a key derived from this one. The journal
	/// keeps it to tell a resent event from another under the same id; a plain hash would
	/// let anyone with a dump confirm a guess at the PII in it (a phone number has few
	/// enough possibilities to try them all).
	pub fn content_mac(&self, canonical: &[u8]) -> [u8; 32] {
		let derived = self.derive(b"sa-panel/content-mac/v1");
		// HMAC takes a key of any length, so this cannot fail.
		let mut mac = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(derived.as_slice()).expect("HMAC accepts any key length");
		mac.update(canonical);
		mac.finalize().into_bytes().into()
	}

	/// A key for one purpose, named by `label`, that says nothing of this one or the others.
	fn derive(&self, label: &[u8]) -> Zeroizing<[u8; 32]> {
		let mut derived = Zeroizing::new([0u8; 32]);
		let mut h = Sha256::new();
		h.update(label);
		h.update(self.key.as_slice());
		derived.copy_from_slice(&h.finalize());
		derived
	}

	/// `nonce(24) || ciphertext+tag`.
	pub fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SealError> {
		let mut nonce = [0u8; NONCE_LEN];
		getrandom::fill(&mut nonce).map_err(|_| SealError::Random)?;
		let ct = self.cipher().encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad }).map_err(|_| SealError::Seal)?;
		let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
		out.extend_from_slice(&nonce);
		out.extend_from_slice(&ct);
		Ok(out)
	}

	pub fn open(&self, aad: &[u8], blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, SealError> {
		if blob.len() < NONCE_LEN {
			return Err(SealError::Open);
		}
		let (nonce, ct) = blob.split_at(NONCE_LEN);
		self.cipher()
			.decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
			.map(Zeroizing::new)
			.map_err(|_| SealError::Open)
	}

	fn cipher(&self) -> XChaCha20Poly1305 {
		XChaCha20Poly1305::new(Key::from_slice(self.key.as_slice()))
	}
}

impl std::fmt::Debug for DataKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DataKey").field("fingerprint", &hex::encode(&self.fingerprint()[..4])).finish_non_exhaustive()
	}
}

/// Associated data of an event's PII.
pub fn pii_aad(event_id: uuid::Uuid) -> Vec<u8> {
	[b"sa-panel/pii/v1/".as_slice(), event_id.as_bytes()].concat()
}

/// Associated data of a note's texts.
pub fn note_aad(rule: &str, event_id: uuid::Uuid) -> Vec<u8> {
	[b"sa-panel/note/v1/".as_slice(), rule.as_bytes(), b"/", event_id.as_bytes()].concat()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_trip_and_binding() {
		let key = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		let blob = key.seal(b"row-1", b"+33 6 00 00 00 00").unwrap();
		assert_eq!(key.open(b"row-1", &blob).unwrap().as_slice(), b"+33 6 00 00 00 00");
		assert_eq!(key.open(b"row-2", &blob).unwrap_err(), SealError::Open, "moved to another row");
		let other = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		assert_eq!(other.open(b"row-1", &blob).unwrap_err(), SealError::Open);
		assert_ne!(key.fingerprint(), other.fingerprint());
		assert_ne!(key.seal(b"row-1", b"x").unwrap(), key.seal(b"row-1", b"x").unwrap(), "a fresh nonce each time");
		assert!(!format!("{key:?}").contains(&hex::encode(key.key.as_slice())));
	}

	#[test]
	fn content_macs_are_keyed() {
		let key = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		let other = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		let plain: [u8; 32] = Sha256::digest(b"event").into();
		assert_eq!(key.content_mac(b"event"), key.content_mac(b"event"));
		assert_ne!(key.content_mac(b"event"), other.content_mac(b"event"));
		assert_ne!(key.content_mac(b"event"), plain, "not a bare hash anyone can recompute");
	}

	#[test]
	fn key_format() {
		assert_eq!(DataKey::from_hex("abcd").unwrap_err(), SealError::KeyFormat);
		assert_eq!(DataKey::from_hex(&"zz".repeat(32)).unwrap_err(), SealError::KeyFormat);
	}
}
