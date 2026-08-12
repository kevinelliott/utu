use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub user_agent: String,
    pub platform_family: String,
    pub platform_os: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    pub session_id: Option<String>,
    pub name: Option<String>,
    pub preview: Option<String>,
    pub cwd: Option<String>,
    pub model_provider: Option<String>,
    pub source_kind: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub ephemeral: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRecord {
    pub id: Option<String>,
    pub kind: String,
    /// Provider payload retained for the native projection layer. Utu never logs
    /// this value; it can contain user text, paths, or command output.
    pub payload: Value,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRecord {
    pub id: String,
    pub status: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub items: Vec<ItemRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRecord {
    pub summary: ThreadSummary,
    pub turns: Vec<TurnRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPage {
    pub data: Vec<ThreadSummary>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadListOptions {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub archived: Option<bool>,
    pub cwd: Option<String>,
    pub search_term: Option<String>,
    pub source_kinds: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ApprovalPolicy {
    #[serde(rename = "untrusted")]
    Untrusted,
    #[serde(rename = "on-request")]
    OnRequest,
    #[serde(rename = "never")]
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TurnSandboxPolicy {
    ReadOnly {
        #[serde(default, rename = "networkAccess")]
        network_access: bool,
    },
    WorkspaceWrite {
        #[serde(default, rename = "writableRoots")]
        writable_roots: Vec<String>,
        #[serde(default, rename = "networkAccess")]
        network_access: bool,
        #[serde(default, rename = "excludeSlashTmp")]
        exclude_slash_tmp: bool,
        #[serde(default, rename = "excludeTmpdirEnvVar")]
        exclude_tmpdir_env_var: bool,
    },
    DangerFullAccess,
}

impl Default for TurnSandboxPolicy {
    fn default() -> Self {
        Self::ReadOnly {
            network_access: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StartThreadOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub ephemeral: Option<bool>,
    pub sandbox: Option<SandboxMode>,
    pub approval_policy: Option<ApprovalPolicy>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResumeThreadOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<SandboxMode>,
    pub approval_policy: Option<ApprovalPolicy>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnStartOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_policy: Option<TurnSandboxPolicy>,
    pub approval_policy: Option<ApprovalPolicy>,
    pub client_user_message_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcServerRequestId {
    Number(u64),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeUpdate {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexEvent {
    ThreadStarted {
        thread: ThreadSummary,
    },
    TurnStarted {
        thread_id: String,
        turn: TurnRecord,
    },
    TurnCompleted {
        thread_id: String,
        turn: TurnRecord,
    },
    ItemStarted {
        thread_id: String,
        turn_id: String,
        item: ItemRecord,
    },
    ItemCompleted {
        thread_id: String,
        turn_id: String,
        item: ItemRecord,
    },
    AgentMessageDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    FileChangeOutputDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    FileChangePatchUpdated {
        update: FileChangeUpdate,
    },
    ThreadLifecycle {
        method: String,
        thread_id: Option<String>,
    },
    /// Utu rejects server-initiated requests until a reviewed approval/UI
    /// contract exists. The request params are deliberately not retained.
    ServerRequestRejected {
        id: RpcServerRequestId,
        method: String,
    },
    UnknownNotification {
        method: String,
    },
    MalformedNotification {
        method: String,
    },
    ProtocolWarning {
        code: String,
    },
    ProcessExited,
}

pub(crate) fn parse_server_info(value: &Value) -> Result<ServerInfo, &'static str> {
    let object = value
        .as_object()
        .ok_or("initialize result is not an object")?;
    // Validate codexHome without retaining or exposing the private local path.
    required_string(object.get("codexHome"), "initialize result lacks codexHome")?;
    Ok(ServerInfo {
        user_agent: required_string(object.get("userAgent"), "initialize result lacks userAgent")?
            .to_owned(),
        platform_family: required_string(
            object.get("platformFamily"),
            "initialize result lacks platformFamily",
        )?
        .to_owned(),
        platform_os: required_string(
            object.get("platformOs"),
            "initialize result lacks platformOs",
        )?
        .to_owned(),
    })
}

pub(crate) fn parse_thread_page(value: &Value) -> Result<ThreadPage, &'static str> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("thread/list result lacks data")?
        .iter()
        .map(parse_thread_summary)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ThreadPage {
        data,
        next_cursor: optional_string(value.get("nextCursor")),
        backwards_cursor: optional_string(value.get("backwardsCursor")),
    })
}

pub(crate) fn parse_thread_result(value: &Value) -> Result<ThreadRecord, &'static str> {
    parse_thread_record(value.get("thread").ok_or("thread response lacks thread")?)
}

pub(crate) fn parse_turn_result(value: &Value) -> Result<TurnRecord, &'static str> {
    parse_turn_record(value.get("turn").ok_or("turn response lacks turn")?)
}

fn parse_thread_record(value: &Value) -> Result<ThreadRecord, &'static str> {
    let summary = parse_thread_summary(value)?;
    let turns = value
        .get("turns")
        .and_then(Value::as_array)
        .map(|turns| {
            turns
                .iter()
                .map(parse_turn_record)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(ThreadRecord { summary, turns })
}

fn parse_thread_summary(value: &Value) -> Result<ThreadSummary, &'static str> {
    let object = value.as_object().ok_or("thread is not an object")?;
    Ok(ThreadSummary {
        id: required_string(object.get("id"), "thread lacks id")?.to_owned(),
        session_id: optional_string(object.get("sessionId")),
        name: optional_string(object.get("name")),
        preview: optional_string(object.get("preview")),
        cwd: optional_string(object.get("cwd")),
        model_provider: optional_string(object.get("modelProvider")),
        source_kind: compact_kind(object.get("source")),
        status: compact_kind(object.get("status")),
        created_at: object.get("createdAt").and_then(Value::as_i64),
        updated_at: object.get("updatedAt").and_then(Value::as_i64),
        ephemeral: object.get("ephemeral").and_then(Value::as_bool),
    })
}

fn parse_turn_record(value: &Value) -> Result<TurnRecord, &'static str> {
    let object = value.as_object().ok_or("turn is not an object")?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(parse_item_record).collect())
        .unwrap_or_default();
    Ok(TurnRecord {
        id: required_string(object.get("id"), "turn lacks id")?.to_owned(),
        status: compact_kind(object.get("status")),
        started_at: object.get("startedAt").and_then(Value::as_i64),
        completed_at: object.get("completedAt").and_then(Value::as_i64),
        duration_ms: object.get("durationMs").and_then(Value::as_i64),
        items,
    })
}

fn parse_item_record(value: &Value) -> ItemRecord {
    ItemRecord {
        id: optional_string(value.get("id")),
        kind: value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        payload: value.clone(),
    }
}

pub(crate) fn parse_notification(method: &str, params: &Value) -> CodexEvent {
    let malformed = || CodexEvent::MalformedNotification {
        method: method.to_owned(),
    };

    match method {
        "thread/started" => params
            .get("thread")
            .and_then(|thread| parse_thread_summary(thread).ok())
            .map(|thread| CodexEvent::ThreadStarted { thread })
            .unwrap_or_else(malformed),
        "turn/started" | "turn/completed" => {
            let thread_id = optional_string(params.get("threadId"));
            let turn = params
                .get("turn")
                .and_then(|turn| parse_turn_record(turn).ok());
            match (thread_id, turn, method) {
                (Some(thread_id), Some(turn), "turn/started") => {
                    CodexEvent::TurnStarted { thread_id, turn }
                }
                (Some(thread_id), Some(turn), _) => CodexEvent::TurnCompleted { thread_id, turn },
                _ => malformed(),
            }
        }
        "item/started" | "item/completed" => {
            let thread_id = optional_string(params.get("threadId"));
            let turn_id = optional_string(params.get("turnId"));
            let item = params.get("item").map(parse_item_record);
            match (thread_id, turn_id, item, method) {
                (Some(thread_id), Some(turn_id), Some(item), "item/started") => {
                    CodexEvent::ItemStarted {
                        thread_id,
                        turn_id,
                        item,
                    }
                }
                (Some(thread_id), Some(turn_id), Some(item), _) => CodexEvent::ItemCompleted {
                    thread_id,
                    turn_id,
                    item,
                },
                _ => malformed(),
            }
        }
        "item/agentMessage/delta" | "item/fileChange/outputDelta" => {
            let thread_id = optional_string(params.get("threadId"));
            let turn_id = optional_string(params.get("turnId"));
            let item_id = optional_string(params.get("itemId"));
            let delta = optional_string(params.get("delta"));
            match (thread_id, turn_id, item_id, delta, method) {
                (
                    Some(thread_id),
                    Some(turn_id),
                    Some(item_id),
                    Some(delta),
                    "item/agentMessage/delta",
                ) => CodexEvent::AgentMessageDelta {
                    thread_id,
                    turn_id,
                    item_id,
                    delta,
                },
                (Some(thread_id), Some(turn_id), Some(item_id), Some(delta), _) => {
                    CodexEvent::FileChangeOutputDelta {
                        thread_id,
                        turn_id,
                        item_id,
                        delta,
                    }
                }
                _ => malformed(),
            }
        }
        "item/fileChange/patchUpdated" => {
            let thread_id = optional_string(params.get("threadId"));
            let turn_id = optional_string(params.get("turnId"));
            let item_id = optional_string(params.get("itemId"));
            let changes = params.get("changes").and_then(Value::as_array).cloned();
            match (thread_id, turn_id, item_id, changes) {
                (Some(thread_id), Some(turn_id), Some(item_id), Some(changes)) => {
                    CodexEvent::FileChangePatchUpdated {
                        update: FileChangeUpdate {
                            thread_id,
                            turn_id,
                            item_id,
                            changes,
                        },
                    }
                }
                _ => malformed(),
            }
        }
        "thread/archived" | "thread/unarchived" | "thread/deleted" | "thread/closed" => {
            CodexEvent::ThreadLifecycle {
                method: method.to_owned(),
                thread_id: optional_string(params.get("threadId")),
            }
        }
        _ => CodexEvent::UnknownNotification {
            method: method.to_owned(),
        },
    }
}

fn required_string<'a>(
    value: Option<&'a Value>,
    error: &'static str,
) -> Result<&'a str, &'static str> {
    value.and_then(Value::as_str).ok_or(error)
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn compact_kind(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(kind) = value.as_str() {
        return Some(kind.to_owned());
    }
    let object = value.as_object()?;
    object
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| object.keys().next().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn thread_projection_drops_unstable_rollout_path_but_keeps_items() {
        let record = parse_thread_result(&json!({
            "thread": {
                "id": "thr_1",
                "path": "/private/rollout.jsonl",
                "preview": "hello",
                "status": {"type": "idle"},
                "turns": [{
                    "id": "turn_1",
                    "status": "completed",
                    "items": [{"id": "item_1", "type": "agentMessage", "text": "done"}]
                }]
            }
        }))
        .unwrap();

        assert_eq!(record.summary.id, "thr_1");
        assert_eq!(record.summary.status.as_deref(), Some("idle"));
        assert_eq!(record.turns[0].items[0].kind, "agentMessage");
        assert!(
            !serde_json::to_string(&record)
                .unwrap()
                .contains("rollout.jsonl")
        );
    }

    #[test]
    fn unknown_notifications_do_not_retain_unreviewed_payloads() {
        let event = parse_notification(
            "account/private/changed",
            &json!({"accessToken": "do-not-retain", "email": "owner@example.com"}),
        );
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("do-not-retain"));
        assert!(!serialized.contains("owner@example.com"));
    }

    #[test]
    fn sandbox_policy_matches_app_server_camel_case_schema() {
        let value = serde_json::to_value(TurnSandboxPolicy::WorkspaceWrite {
            writable_roots: vec!["/workspace".into()],
            network_access: false,
            exclude_slash_tmp: true,
            exclude_tmpdir_env_var: true,
        })
        .unwrap();
        assert_eq!(value["type"], "workspaceWrite");
        assert_eq!(value["writableRoots"][0], "/workspace");
        assert_eq!(value["excludeSlashTmp"], true);
    }
}
