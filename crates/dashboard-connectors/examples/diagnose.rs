fn main() {
    let report = utu_connectors::diagnose_known_connectors();
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize connector diagnostics: {error}");
            std::process::exit(1);
        }
    }
}
