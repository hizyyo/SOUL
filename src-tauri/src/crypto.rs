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
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;
pub const SIG_LEN: usize = 64;
pub const LOCAL_SECRET_LEN: usize = 32;

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
    if dir.exists() {
        if !secret_path.is_file() || !public_path.is_file() {
            return Err(
                "Device keypair is incomplete; refusing to replace or regenerate it.".into(),
            );
        }
        let secret = read_secret_file(&secret_path)?;
        let public = fs::read(&public_path).map_err(|e| format!("Cannot read device key: {e}"))?;
        if secret.len() != 32 || public.len() != 32 {
            return Err("Device key files are corrupted (wrong length).".into());
        }
        let private_bytes: [u8; 32] = secret
            .try_into()
            .map_err(|_| "Corrupted device key.".to_string())?;
        let expected_public = SigningKey::from_bytes(&private_bytes)
            .verifying_key()
            .to_bytes();
        if public.as_slice() != expected_public {
            return Err("Device key files do not form a valid keypair.".into());
        }
        return Ok(DeviceKeys {
            public_b64: B64.encode(public),
            private_bytes,
        });
    }

    fs::create_dir_all(app_dir).map_err(|e| format!("Cannot create app dir: {e}"))?;
    let signing_key = SigningKey::generate(&mut OsRng);
    let secret = signing_key.to_bytes();
    let public = signing_key.verifying_key().to_bytes();
    let tmp_dir = app_dir.join(format!(".keys.tmp-{}", Uuid::new_v4()));
    fs::create_dir(&tmp_dir).map_err(|e| format!("Cannot create temporary keys dir: {e}"))?;
    let result = (|| {
        write_secret_file(&tmp_dir.join("device_ed25519.secret"), &secret)?;
        write_key_file(&tmp_dir.join("device_ed25519.pub"), &public)?;
        fs::rename(&tmp_dir, &dir).map_err(|e| format!("Cannot install device keypair: {e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&tmp_dir);
    }
    result?;

    Ok(DeviceKeys {
        public_b64: B64.encode(public),
        private_bytes: secret,
    })
}

fn local_secret_path(app_dir: &Path) -> PathBuf {
    keys_dir(app_dir).join("mcp_capability.secret")
}

pub fn ensure_local_capability_secret(app_dir: &Path) -> Result<String, String> {
    let _ = ensure_device_keypair(app_dir)?;
    let path = local_secret_path(app_dir);
    if path.exists() {
        let bytes = read_secret_file(&path)?;
        if bytes.len() != LOCAL_SECRET_LEN {
            return Err("Local capability secret is corrupted.".to_string());
        }
        return Ok(hex::encode(bytes));
    }
    let mut secret = [0u8; LOCAL_SECRET_LEN];
    OsRng.fill_bytes(&mut secret);
    let tmp = path.with_extension(format!("secret.tmp-{}", Uuid::new_v4()));
    write_secret_file(&tmp, &secret)?;
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        if path.exists() {
            return ensure_local_capability_secret(app_dir);
        }
        return Err(format!("Cannot install local capability secret: {e}"));
    }
    Ok(hex::encode(secret))
}

/// Replaces the local MCP authorization secret. Existing client configurations
/// retain the old value and therefore fail authorization until reconnected.
pub fn rotate_local_capability_secret(app_dir: &Path) -> Result<String, String> {
    let _ = ensure_device_keypair(app_dir)?;
    let path = local_secret_path(app_dir);
    let mut secret = [0u8; LOCAL_SECRET_LEN];
    OsRng.fill_bytes(&mut secret);
    write_secret_file(&path, &secret)?;
    Ok(hex::encode(secret))
}

pub fn verify_local_capability_secret(app_dir: &Path, supplied: &str) -> Result<(), String> {
    let expected = ensure_local_capability_secret(app_dir)?;
    if supplied.len() != expected.len()
        || !supplied
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            .eq(&0)
    {
        return Err("Local MCP capability authorization failed.".to_string());
    }
    Ok(())
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

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(windows)]
    let stored = protect_for_current_user(bytes)?;
    #[cfg(not(windows))]
    let stored = bytes.to_vec();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("secret");
    let tmp = path.with_file_name(format!(".{file_name}.tmp-{}", Uuid::new_v4()));
    let backup = path.with_file_name(format!(".{file_name}.replace-backup"));
    write_key_file(&tmp, &stored)?;
    if path.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(|e| format!("Cannot preserve local secret: {e}"))?;
    }
    match fs::rename(&tmp, path) {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(e) => {
            let _ = fs::rename(&backup, path);
            let _ = fs::remove_file(&tmp);
            Err(format!("Cannot install local secret: {e}"))
        }
    }
}

fn read_secret_file(path: &Path) -> Result<Vec<u8>, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("secret");
    let backup = path.with_file_name(format!(".{file_name}.replace-backup"));
    if !path.exists() && backup.exists() {
        fs::rename(&backup, path)
            .map_err(|e| format!("Cannot recover interrupted secret replacement: {e}"))?;
    } else if path.exists() && backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    let stored = fs::read(path).map_err(|e| format!("Cannot read local secret: {e}"))?;
    #[cfg(windows)]
    {
        if stored.len() == KEY_LEN {
            // One-time migration from the legacy raw secret format. The caller
            // validates the secret before using it; a failed rewrite is fatal.
            write_secret_file(path, &stored)?;
            return Ok(stored);
        }
        unprotect_for_current_user(&stored)
    }
    #[cfg(not(windows))]
    {
        Ok(stored)
    }
}

#[cfg(windows)]
fn protect_for_current_user(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "Secret is too large for DPAPI.".to_string())?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input points to `bytes` for the duration of the call; all optional
    // pointers are null; DPAPI owns output.pbData until released with LocalFree.
    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err(format!(
            "Cannot protect local secret with Windows DPAPI: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: DPAPI returned a valid buffer of cbData bytes on success.
    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: output.pbData was allocated by DPAPI and must be released once.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(protected)
}

#[cfg(windows)]
fn unprotect_for_current_user(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "Protected secret is too large for DPAPI.".to_string())?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: pointers follow CryptUnprotectData's contract; the output is
    // copied before being released with LocalFree.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err(format!(
            "Cannot unlock local secret with Windows DPAPI: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: DPAPI returned a valid buffer of cbData bytes on success.
    let plaintext =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: output.pbData was allocated by DPAPI and must be released once.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(plaintext)
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

/// Ключ шифрования локальной SQLite-БД (SQLCipher): SHA-256 от приватного
/// ключа устройства. Пока существуют файлы device keypair — БД расшифровывается;
/// отсутствие ключей (например, после полного wipe) делает данные недоступными,
/// что и требуется по §4.1.
pub fn db_encryption_key(app_dir: &Path) -> Result<[u8; KEY_LEN], String> {
    let keys = ensure_device_keypair(app_dir)?;
    let mut hasher = Sha256::new();
    hasher.update(keys.private_bytes);
    Ok(hasher.finalize().into())
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
    if path.to_string_lossy().contains('\0') {
        return Err("File path must not contain NUL characters.".to_string());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-crypto-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn incomplete_or_mismatched_keypair_fails_closed() {
        let dir = temp_dir();
        fs::create_dir_all(keys_dir(&dir)).unwrap();
        fs::write(key_paths(&dir).0, [1u8; 32]).unwrap();
        assert!(ensure_device_keypair(&dir)
            .unwrap_err()
            .contains("incomplete"));

        fs::write(key_paths(&dir).1, [2u8; 32]).unwrap();
        assert!(ensure_device_keypair(&dir)
            .unwrap_err()
            .contains("valid keypair"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn capability_secret_is_stable_and_checked() {
        let dir = temp_dir();
        let first = ensure_local_capability_secret(&dir).unwrap();
        assert_eq!(first, ensure_local_capability_secret(&dir).unwrap());
        verify_local_capability_secret(&dir, &first).unwrap();
        assert!(verify_local_capability_secret(&dir, &"0".repeat(first.len())).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rotating_capability_secret_revokes_the_previous_value() {
        let dir = temp_dir();
        let old = ensure_local_capability_secret(&dir).unwrap();
        let new = rotate_local_capability_secret(&dir).unwrap();
        assert_ne!(old, new);
        assert!(verify_local_capability_secret(&dir, &new).is_ok());
        assert!(verify_local_capability_secret(&dir, &old).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
