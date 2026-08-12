use std::{env, process::ExitCode};

use utu_codex::{CodexClient, ThreadListOptions};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Codex App Server read-only check failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = CodexClient::connect_default()?;
    let page = client.list_threads(ThreadListOptions {
        limit: Some(5),
        ..ThreadListOptions::default()
    })?;
    println!(
        "Codex App Server {} / {}; {} thread summaries observed",
        client.server_info().platform_family,
        client.server_info().platform_os,
        page.data.len()
    );
    if env::args().any(|argument| argument == "--ids") {
        for thread in page.data {
            println!("{}", thread.id);
        }
    }
    client.shutdown()?;
    Ok(())
}
