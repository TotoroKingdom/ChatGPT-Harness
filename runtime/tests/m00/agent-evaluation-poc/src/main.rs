use m00_evaluation_poc::{
    run_case, EvalCase, HttpJsonAgentAdapter, HttpMcpTransport, WorkspaceFinalStateProbe,
    RUNNER_VERSION,
};
use std::{env, process};

fn print_help() {
    println!(
        "{RUNNER_VERSION}\nUsage:\n  m00-evaluation-poc --case CASE.json --mcp-url http://127.0.0.1:PORT/mcp --adapter-url http://127.0.0.1:PORT/turn --workspace-root PATH\n\nThe adapter endpoint must implement the provider-neutral JSON turn contract.\nAn optional M00_AGENT_AUTH_TOKEN environment variable is sent only as a Bearer header and is never written to the report."
    );
}

fn argument(name: &str, args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }
    let Some(case_path) = argument("--case", &args) else {
        print_help();
        process::exit(2);
    };
    let Some(mcp_url) = argument("--mcp-url", &args) else {
        print_help();
        process::exit(2);
    };
    let Some(adapter_url) = argument("--adapter-url", &args) else {
        print_help();
        process::exit(2);
    };
    let Some(workspace_root) = argument("--workspace-root", &args) else {
        print_help();
        process::exit(2);
    };

    let case = match EvalCase::load(&case_path) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };
    let token = env::var("M00_AGENT_AUTH_TOKEN").ok();
    let mut transport = match HttpMcpTransport::new(mcp_url) {
        Ok(transport) => transport,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };
    let mut adapter = match HttpJsonAgentAdapter::new(adapter_url, case.adapter_id.clone(), token) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };
    let probe = WorkspaceFinalStateProbe::new(workspace_root);
    match run_case(&case, &mut transport, &mut adapter, &probe) {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
