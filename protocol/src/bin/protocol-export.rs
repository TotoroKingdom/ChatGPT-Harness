use harness_control_protocol::export;
use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(command) = env::args().nth(1) else {
        eprintln!("usage: protocol-export <generate-ts|generate-json-schema|generate-all>");
        return ExitCode::from(2);
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = match command.as_str() {
        "generate-ts" => export::generate_typescript(&root),
        "generate-json-schema" => export::generate_json_schema(&root),
        "generate-all" => export::generate_all(&root),
        _ => {
            eprintln!("unknown command: {command}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("protocol generation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
