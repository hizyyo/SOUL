use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;
pub const SIG_LEN: usize = 64;

pub const KDF_MEM_KIB: u32 = 19_456;
pub const KDF_TIME: u32 = 2;
pub const KDF_PARALLELISM: u32 = 1;

#[derive(Debug, Clone)]
pub struct DeviceKeys {
    pub public_b64: String,
    pub private_bytes: [u8; 32],
}

pub fn keys_dir(app_dir: &Path) -> std::path::PathBuf {
    app_dir.join("keys")
}

fn key_paths(app_dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        keys_dir(app_dir).join("device_ed25519.secret"),
        keys_dir(app_dir).join("device_ed25519.pub"),
    )
}

pub fn ensure_device_keypair(app_dir: &Path) -> Result<DeviceKeys, String> {
    let (secret_path, public_path) = key_paths(app_dir);
    let dir = keys_dir(app_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create keys dir: {e}"))?;

    if secret_path.exists() && public_path.exists() {
        let secret = fs::read(&secret_path).map_err(|e| format!("Cannot read device key: {e}"))?;
        let public = fs::read(&public_path).map_err(|e| format!("Cannot read device key: {e}"))?;
        if secret.len() != 32 || public.len() != 32 {
            return Err("Device key files are corrupted (wrong length).".into());
        }
        return Ok(DeviceKeys {
            public_b64: B64.encode(public),
            private_bytes: secret
                .try_into()
                .map_err(|_| "Corrupted device key.".to_string())?,
        });
    }

    let signing_key = SigningKey::generate(&mut OsRng);
    let secret = signing_key.to_bytes();
    let public = signing_key.verifying_key().to_bytes();

    write_key_file(&secret_path, &secret)?;
    write_key_file(&public_path, &public)?;

    Ok(DeviceKeys {
        public_b64: B64.encode(public),
        private_bytes: secret,
    })
}

fn write_key_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("Cannot write device key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn delete_device_keys(app_dir: &Path) -> Result<(), String> {
    let dir = keys_dir(app_dir);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Cannot remove device keys: {e}"))?;
    }
    Ok(())
}

pub fn default_kdf_params() -> (u32, u32, u32) {
    (KDF_MEM_KIB, KDF_TIME, KDF_PARALLELISM)
}

fn derive_key(
    password: &str,
    salt: &[u8],
    mem_kib: u32,
    time: u32,
    p: u32,
) -> Result<[u8; KEY_LEN], String> {
    let params = Params::new(mem_kib, time, p, Some(KEY_LEN))
        .map_err(|e| format!("Invalid KDF params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    Ok(key)
}

#[derive(Debug, Clone)]
pub struct SealedBox {
    pub ciphertext: Vec<u8>,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
}

pub fn encrypt_payload(
    plaintext: &[u8],
    password: &str,
    mem_kib: u32,
    time: u32,
    p: u32,
) -> Result<SealedBox, String> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let key = derive_key(password, &salt, mem_kib, time, p)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| "Encryption failed.".to_string())?;
    Ok(SealedBox {
        ciphertext,
        salt,
        nonce,
    })
}

pub fn decrypt_payload(
    ciphertext: &[u8],
    password: &str,
    salt: &[u8],
    nonce: &[u8],
    mem_kib: u32,
    time: u32,
    p: u32,
) -> Result<Vec<u8>, String> {
    let key = derive_key(password, salt, mem_kib, time, p)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| "Incorrect passphrase or corrupted payload.".to_string())
}

pub fn sign_bytes(private_bytes: &[u8; 32], msg: &[u8]) -> [u8; SIG_LEN] {
    let signing_key = SigningKey::from_bytes(private_bytes);
    let sig: Signature = signing_key.sign(msg);
    sig.to_bytes()
}

pub fn verify_signature(public_b64: &str, msg: &[u8], sig_bytes: &[u8]) -> bool {
    let public = match B64.decode(public_b64).ok() {
        Some(p) => p,
        None => return false,
    };
    let public: [u8; 32] = match public.try_into() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let verifying_key = match VerifyingKey::from_bytes(&public) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig_bytes: [u8; SIG_LEN] = match sig_bytes.try_into() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_bytes);
    verifying_key.verify_strict(msg, &signature).is_ok()
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn ensure_password_valid(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Passphrase must be at least 8 characters.".into());
    }
    if password.len() > 512 {
        return Err("Passphrase is too long.".into());
    }
    Ok(())
}

pub fn read_file_limited(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Cannot read file: {e}"))?;
    if meta.len() as usize > max_bytes {
        return Err(format!("File is too large (limit {max_bytes} bytes)."));
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read file: {e}"))?;
    if bytes.len() > max_bytes {
        return Err(format!("File is too large (limit {max_bytes} bytes)."));
    }
    Ok(bytes)
}
