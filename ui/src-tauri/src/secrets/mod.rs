use harness_control_protocol::SecretReference;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn exists(&self, key: &str) -> Result<bool, String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}
impl SecretStore for MemorySecretStore {
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        self.values
            .lock()
            .map_err(|e| e.to_string())?
            .insert(k.into(), v.into());
        Ok(())
    }
    fn exists(&self, k: &str) -> Result<bool, String> {
        Ok(self
            .values
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(k))
    }
    fn delete(&self, k: &str) -> Result<(), String> {
        self.values.lock().map_err(|e| e.to_string())?.remove(k);
        Ok(())
    }
}

#[cfg(windows)]
pub struct WindowsCredentialStore;
#[cfg(windows)]
impl SecretStore for WindowsCredentialStore {
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        keyring::Entry::new("ChatGPT-Harness", k)
            .map_err(|e| e.to_string())?
            .set_password(v)
            .map_err(|e| e.to_string())
    }
    fn exists(&self, k: &str) -> Result<bool, String> {
        match keyring::Entry::new("ChatGPT-Harness", k)
            .map_err(|e| e.to_string())?
            .get_password()
        {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
    fn delete(&self, k: &str) -> Result<(), String> {
        match keyring::Entry::new("ChatGPT-Harness", k)
            .map_err(|e| e.to_string())?
            .delete_credential()
        {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub fn api_key_ref() -> SecretReference {
    SecretReference {
        provider: if cfg!(windows) {
            "windows-credential-manager".into()
        } else {
            "memory-development".into()
        },
        key: "tunnel-api-key".into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_secret_store_replaces_and_deletes_values() {
        let store = MemorySecretStore::default();
        store.set("api-key", "first-secret").unwrap();
        assert!(store.exists("api-key").unwrap());
        store.set("api-key", "replacement-secret").unwrap();
        assert!(store.exists("api-key").unwrap());
        store.delete("api-key").unwrap();
        assert!(!store.exists("api-key").unwrap());
    }

    #[test]
    fn secret_reference_contains_only_locator_data() {
        let reference = api_key_ref();
        let serialized = serde_json::to_string(&reference).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(serialized.contains("tunnel-api-key"));
    }
}
