use harness_control_protocol::SecretReference;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
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
    fn get(&self, k: &str) -> Result<Option<String>, String> {
        Ok(self
            .values
            .lock()
            .map_err(|e| e.to_string())?
            .get(k)
            .cloned())
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

/// The dedicated QA build uses only ephemeral synthetic credentials, never
/// the real user's Windows Credential Manager. Normal builds omit this code.
#[cfg(feature = "qa-isolated-credentials")]
pub(crate) fn qa_fixture_secrets(present: bool) -> Box<dyn SecretStore> {
    let store = MemorySecretStore::default();
    if present {
        store
            .set("tunnel-api-key", "m07-synthetic-fixture")
            .expect("memory fixture");
    }
    Box::new(store)
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
    fn get(&self, k: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new("ChatGPT-Harness", k)
            .map_err(|e| e.to_string())?
            .get_password()
        {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
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

    #[cfg(feature = "qa-isolated-credentials")]
    #[test]
    fn isolated_qa_credentials_are_ephemeral_and_can_be_missing() {
        let missing = qa_fixture_secrets(false);
        assert!(!missing.exists("tunnel-api-key").unwrap());
        let present = qa_fixture_secrets(true);
        assert!(present.exists("tunnel-api-key").unwrap());
        present
            .set("tunnel-api-key", "another-synthetic-value")
            .unwrap();
        assert!(!qa_fixture_secrets(false).exists("tunnel-api-key").unwrap());
    }

    #[test]
    fn memory_secret_store_replaces_and_deletes_values() {
        let store = MemorySecretStore::default();
        store.set("api-key", "first-secret").unwrap();
        assert!(store.exists("api-key").unwrap());
        assert_eq!(
            store.get("api-key").unwrap().as_deref(),
            Some("first-secret")
        );
        store.set("api-key", "replacement-secret").unwrap();
        assert!(store.exists("api-key").unwrap());
        assert_eq!(
            store.get("api-key").unwrap().as_deref(),
            Some("replacement-secret")
        );
        store.delete("api-key").unwrap();
        assert!(!store.exists("api-key").unwrap());
        assert_eq!(store.get("api-key").unwrap(), None);
    }

    #[test]
    fn secret_reference_contains_only_locator_data() {
        let reference = api_key_ref();
        let serialized = serde_json::to_string(&reference).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(serialized.contains("tunnel-api-key"));
    }
}
