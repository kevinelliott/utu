//! A bounded, local Codex App Server transport for Utu.
//!
//! The adapter speaks the v2 JSON-RPC-over-stdio surface exposed by the
//! experimental `codex app-server` command. It intentionally does not inspect
//! Codex credential files, log protocol payloads, auto-approve requests, or
//! infer costs.

#![forbid(unsafe_code)]

mod transport;
mod types;

pub use transport::{ClientConfig, CodexClient, CodexError, NotificationPolicy, StderrStats};
pub use types::{
    ApprovalPolicy, CodexEvent, FileChangeUpdate, ItemRecord, ResumeThreadOptions,
    RpcServerRequestId, SandboxMode, ServerInfo, StartThreadOptions, ThreadListOptions, ThreadPage,
    ThreadRecord, ThreadSummary, TurnRecord, TurnSandboxPolicy, TurnStartOptions,
};
