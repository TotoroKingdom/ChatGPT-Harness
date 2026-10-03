//! Small subprocess bridge for the M6 production-frontend E2E runner.
//!
//! This example deliberately reuses the production `ConfigStore` and platform
//! normalization code by including the source modules directly.  It is not a
//! second config implementation and it does not alter the Tauri command
//! registration.  The runner invokes this executable for each command through
//! the same command names used by `TauriHarnessControlClient`.

#[path = "../src/config/mod.rs"]
mod config;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/runtime/mod.rs"]
mod runtime;

// `runtime/control_read.rs` only needs this conversion while compiling the
// production RuntimeSupervisor module into the example.  The actual Tauri
// command conversion has the same wire shape in commands/mod.rs.
mod commands {
    use harness_control_protocol::control_read::ReadErrorCode;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct ControlError {
        pub code: String,
        pub message: String,
    }

    impl From<ReadErrorCode> for ControlError {
        fn from(error: ReadErrorCode) -> Self {
            Self {
                code: error.as_str().into(),
                message: error.message().into(),
            }
        }
    }
}

use config::{ConfigError, ConfigStore};
use harness_control_protocol::{AppConfig, UiLanguage};
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf, process};

fn argument(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

fn fail(code: &str, message: impl Into<String>) -> ! {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "ok": false,
            "code": code,
            "message": message.into(),
        }))
        .expect("bridge error is serializable")
    );
    process::exit(0);
}

fn config_error(error: ConfigError) -> ! {
    fail(error.code(), error.message());
}

fn main() {
    let directory = argument("--dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("BRIDGE_INPUT", "--dir is required"));
    let operation = argument("--op").unwrap_or_else(|| fail("BRIDGE_INPUT", "--op is required"));
    let store = ConfigStore::new(directory.clone());

    let value = match operation.as_str() {
        "get" => match store.load_or_create_defaults() {
            Ok(config) => serde_json::to_value(config).expect("AppConfig is serializable"),
            Err(error) => config_error(error),
        },
        "load" => match store.load() {
            Ok(config) => serde_json::to_value(config).expect("AppConfig is serializable"),
            Err(error) => config_error(error),
        },
        "metadata" => serde_json::to_value(store.metadata()).expect("metadata is serializable"),
        "reset" => {
            if let Err(error) = store.reset() {
                config_error(error);
            }
            Value::Null
        }
        "set" => {
            let language = match argument("--language").as_deref() {
                None | Some("null") => None,
                Some("en") => Some(UiLanguage::En),
                Some("zh-CN") => Some(UiLanguage::ZhCn),
                Some(value) => fail("BRIDGE_INPUT", format!("unsupported language: {value}")),
            };
            match store.set_ui_language(language) {
                Ok(config) => serde_json::to_value(config).expect("AppConfig is serializable"),
                Err(error) => config_error(error),
            }
        }
        "save" => {
            let encoded = argument("--config-json")
                .unwrap_or_else(|| fail("BRIDGE_INPUT", "--config-json is required"));
            let config: AppConfig = serde_json::from_str(&encoded).unwrap_or_else(|error| {
                fail("BRIDGE_INPUT", format!("invalid AppConfig: {error}"))
            });
            match store.save(config) {
                Ok(config) => serde_json::to_value(config).expect("AppConfig is serializable"),
                Err(error) => config_error(error),
            }
        }
        "block-temp" => {
            let temp = directory.join("config.json.tmp");
            if let Err(error) = fs::create_dir_all(&temp) {
                fail("BRIDGE_IO", error.to_string());
            }
            Value::Null
        }
        "unblock-temp" => {
            let temp = directory.join("config.json.tmp");
            if temp.is_dir() {
                if let Err(error) = fs::remove_dir(temp) {
                    fail("BRIDGE_IO", error.to_string());
                }
            } else if temp.exists() {
                if let Err(error) = fs::remove_file(temp) {
                    fail("BRIDGE_IO", error.to_string());
                }
            }
            Value::Null
        }
        // Exercise the same production RuntimeSupervisor -> control-read
        // mapping used by the Tauri command. With no child process attached,
        // RuntimeSupervisor produces RUNTIME_DISCONNECTED, which the typed
        // read boundary maps to RUNTIME_UNAVAILABLE.
        "control-error" => {
            let supervisor = runtime::RuntimeSupervisor::new();
            match supervisor.get_effective_tools() {
                Ok(value) => serde_json::to_value(value).expect("control read is serializable"),
                Err(error) => fail(error.as_str(), error.message()),
            }
        }
        _ => fail(
            "BRIDGE_INPUT",
            format!("unsupported operation: {operation}"),
        ),
    };

    println!(
        "{}",
        serde_json::to_string(&json!({ "ok": true, "value": value }))
            .expect("bridge response is serializable")
    );
}
