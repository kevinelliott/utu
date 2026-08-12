use std::{
    env,
    io::{self, BufRead, Write},
    process,
};

fn secondary_argument() -> String {
    env::args().nth(2).expect("secondary argument")
}

fn send(value: &str) {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}").unwrap();
    stdout.flush().unwrap();
}

fn next_line(lines: &mut impl Iterator<Item = io::Result<String>>) -> String {
    lines
        .next()
        .expect("client closed input")
        .expect("read input")
}

fn handshake(lines: &mut impl Iterator<Item = io::Result<String>>) {
    let initialize = next_line(lines);
    assert!(initialize.contains("\"method\":\"initialize\""));
    send(
        r#"{"id":1,"result":{"codexHome":"/private/not-retained","platformFamily":"unix","platformOs":"test","userAgent":"codex-test"}}"#,
    );
    let initialized = next_line(lines);
    assert!(initialized.contains("\"method\":\"initialized\""));
}

fn main() {
    let scenario = env::args().nth(1).expect("scenario");
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    handshake(&mut lines);

    match scenario.as_str() {
        "vertical" => {
            for (method, response) in [
                (
                    "thread/list",
                    r#"{"id":2,"result":{"data":[{"id":"thr_list","preview":"listed","turns":[]}],"nextCursor":null}}"#,
                ),
                (
                    "thread/read",
                    r#"{"id":3,"result":{"thread":{"id":"thr_list","turns":[{"id":"turn_old","status":"completed","items":[]}]}}}"#,
                ),
                (
                    "thread/resume",
                    r#"{"id":4,"result":{"thread":{"id":"thr_list","turns":[]}}}"#,
                ),
                (
                    "thread/start",
                    r#"{"id":5,"result":{"thread":{"id":"thr_new","turns":[]}}}"#,
                ),
                (
                    "turn/start",
                    r#"{"id":6,"result":{"turn":{"id":"turn_new","status":"inProgress","items":[]}}}"#,
                ),
            ] {
                let request = next_line(&mut lines);
                assert!(request.contains(&format!("\"method\":\"{method}\"")));
                if method == "turn/start" {
                    assert!(request.contains("\"text\":\"owner direction\""));
                }
                send(response);
            }
            for _ in lines {}
        }
        "interleaved" => {
            let _request = next_line(&mut lines);
            send(
                r#"{"method":"item/agentMessage/delta","params":{"threadId":"thr_1","turnId":"turn_1","itemId":"item_1","delta":"hello"}}"#,
            );
            send(r#"{"id":2,"result":{"data":[]}}"#);
        }
        "metadata-secrets" => {
            let _request = next_line(&mut lines);
            send(
                r#"{"method":"thread/started","params":{"thread":{"id":"thr_private","name":"private-name","preview":"private-preview","turns":[]}}}"#,
            );
            send(
                r#"{"method":"turn/completed","params":{"threadId":"thr_private","turn":{"id":"turn_private","status":"completed","items":[{"id":"item_private","type":"agentMessage","text":"private-turn-payload"}]}}}"#,
            );
            send(
                r#"{"method":"account/updated","params":{"accessToken":"private-account-token"}}"#,
            );
            send(r#"{"id":2,"result":{"data":[]}}"#);
            for _ in lines {}
        }
        "rpc-error" => {
            let _request = next_line(&mut lines);
            send(r#"{"id":2,"error":{"code":77,"message":"owner@example.com token=private"}}"#);
        }
        "malformed" => {
            let _request = next_line(&mut lines);
            send("{not-json");
        }
        "malformed-descendant" => {
            let pid_file = secondary_argument();
            let mut child = process::Command::new("sleep").arg("30").spawn().unwrap();
            std::fs::write(pid_file, child.id().to_string()).unwrap();
            let _request = next_line(&mut lines);
            send("{not-json");
            let _ = child.wait();
        }
        "invalid-response-descendant" => {
            let pid_file = secondary_argument();
            let mut child = process::Command::new("sleep").arg("30").spawn().unwrap();
            std::fs::write(pid_file, child.id().to_string()).unwrap();
            let _request = next_line(&mut lines);
            send(r#"{"id":2,"result":{},"error":{"code":77}}"#);
            let _ = child.wait();
        }
        "timeout" => {
            let _request = next_line(&mut lines);
            for _ in lines {}
        }
        "exit" => {}
        "server-request" => {
            send(
                r#"{"id":"approval-1","method":"item/commandExecution/requestApproval","params":{"accessToken":"private-token","command":"dangerous"}}"#,
            );
            let rejection = next_line(&mut lines);
            assert!(rejection.contains("\"code\":-32601"));
            let _request = next_line(&mut lines);
            send(r#"{"id":2,"result":{"data":[]}}"#);
        }
        "oversized" => {
            let _request = next_line(&mut lines);
            send(&"x".repeat(2_048));
        }
        "stderr" => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            for _ in 0..4_096 {
                stderr.write_all(b"private-stderr-token-").unwrap();
            }
            stderr.flush().unwrap();
            for _ in lines {}
        }
        "graceful" => {
            let marker = secondary_argument();
            for _ in lines {}
            std::fs::write(marker, "stopped").unwrap();
        }
        "descendant" => {
            let pid_file = secondary_argument();
            let mut child = process::Command::new("sleep").arg("30").spawn().unwrap();
            std::fs::write(pid_file, child.id().to_string()).unwrap();
            for _ in lines {}
            let _ = child.wait();
        }
        other => panic!("unknown scenario {other}"),
    }
}
