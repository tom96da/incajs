// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON-RPC 2.0 over newline-delimited JSON, the dev protocol this host
//! speaks on stdin/stdout.

use inca_jsenv::EngineError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const JSONRPC: &str = "2.0";

/// The revision of this host's own method set, reported in the `ready`
/// notification. Bump it whenever a change would break a client written
/// against the old shape.
pub const PROTOCOL_VERSION: u32 = 0;

/// A method name this host answers. An unrecognized one still decodes, so a
/// newer client can call methods this build predates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    Reload,
    Shutdown,
    #[serde(other)]
    Unknown,
}

/// One decoded stdin line. `id` is absent for a notification, which takes no
/// response; a request's response must echo it back unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Call { id: Option<Value>, method: Method },
    Empty,
}

/// The reserved codes this host uses, plus one from the range JSON-RPC
/// leaves to the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    BundleFailed,
}

impl ErrorCode {
    fn code(self) -> i32 {
        match self {
            ErrorCode::ParseError => -32700,
            ErrorCode::InvalidRequest => -32600,
            ErrorCode::MethodNotFound => -32601,
            ErrorCode::BundleFailed => -32000,
        }
    }
}

/// What JSON-RPC leaves to the application, used for the half of a thrown
/// JS value that isn't its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorData {
    stack: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<ErrorData>,
}

/// A line this host writes back.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Outgoing {
    Result {
        jsonrpc: &'static str,
        id: Value,
        result: Value,
    },
    Failure {
        jsonrpc: &'static str,
        id: Value,
        error: RpcError,
    },
    Notification {
        jsonrpc: &'static str,
        method: &'static str,
        params: Value,
    },
}

impl Outgoing {
    /// A success response. `id` is the request's, echoed unchanged.
    #[must_use]
    pub fn result(id: Value) -> Self {
        Outgoing::Result {
            jsonrpc: JSONRPC,
            id,
            result: Value::Null,
        }
    }

    /// An error response. `id` is [`Value::Null`] when the request was too
    /// malformed to read one out of, as JSON-RPC requires.
    #[must_use]
    pub fn error(id: Value, code: ErrorCode, message: impl Into<String>) -> Self {
        Outgoing::Failure {
            jsonrpc: JSONRPC,
            id,
            error: RpcError {
                code: code.code(),
                message: message.into(),
                data: None,
            },
        }
    }

    /// An error response carrying a thrown JS value, split the way
    /// `appError` splits one: the value as text, and its frames beside it.
    #[must_use]
    pub fn thrown(id: Value, code: ErrorCode, thrown: &EngineError) -> Self {
        Outgoing::Failure {
            jsonrpc: JSONRPC,
            id,
            error: RpcError {
                code: code.code(),
                message: thrown.message().to_owned(),
                data: Some(ErrorData {
                    stack: thrown.stack().map(str::to_owned),
                }),
            },
        }
    }

    /// The notification reporting a failure inside the running app. Nothing
    /// asked for it, so it answers no `id`.
    #[must_use]
    pub fn app_error(thrown: &EngineError) -> Self {
        Outgoing::Notification {
            jsonrpc: JSONRPC,
            method: "appError",
            params: serde_json::json!({
                "message": thrown.message(),
                "stack": thrown.stack(),
            }),
        }
    }

    /// The notification announcing that the window is up and the first
    /// bundle has been evaluated.
    #[must_use]
    pub fn ready() -> Self {
        Outgoing::Notification {
            jsonrpc: JSONRPC,
            method: "ready",
            params: serde_json::json!({ "protocol": PROTOCOL_VERSION }),
        }
    }
}

/// Keeps an explicit `"id": null` distinct from an absent one. serde maps
/// both to `None` by default, which would turn a request JSON-RPC merely
/// discourages into a notification and leave its client waiting forever.
fn present_id<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
struct RawCall {
    jsonrpc: String,
    #[serde(default, deserialize_with = "present_id")]
    id: Option<Value>,
    method: Method,
}

/// Decodes one line of stdin.
///
/// # Errors
///
/// Returns the code and message to answer with for a line that isn't a JSON
/// object, or is one this host can't read a call out of. Report it and keep
/// reading — a line the host can't use is never a reason to exit.
pub fn decode(line: &str) -> Result<Incoming, (ErrorCode, String)> {
    if line.trim().is_empty() {
        return Ok(Incoming::Empty);
    }

    let value: Value =
        serde_json::from_str(line).map_err(|err| (ErrorCode::ParseError, err.to_string()))?;

    // serde reads a struct out of a sequence too, so without this check
    // `["2.0", 1, "reload"]` decodes as a reload call.
    if !value.is_object() {
        return Err((
            ErrorCode::InvalidRequest,
            "expected a JSON object".to_owned(),
        ));
    }

    let call: RawCall = serde_json::from_value(value)
        .map_err(|err| (ErrorCode::InvalidRequest, err.to_string()))?;
    if call.jsonrpc != JSONRPC {
        return Err((
            ErrorCode::InvalidRequest,
            format!("unsupported jsonrpc version: {}", call.jsonrpc),
        ));
    }
    if let Some(id) = &call.id
        && !matches!(id, Value::Null | Value::String(_) | Value::Number(_))
    {
        return Err((
            ErrorCode::InvalidRequest,
            "id must be a string, a number, or null".to_owned(),
        ));
    }

    Ok(Incoming::Call {
        id: call.id,
        method: call.method,
    })
}

/// Encodes one message as the line to write, newline excluded.
///
/// # Errors
///
/// Returns an error if `message` fails to serialize.
pub fn encode(message: &Outgoing) -> serde_json::Result<String> {
    serde_json::to_string(message)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(line: &str) -> Incoming {
        decode(line).unwrap()
    }

    #[test]
    fn decodes_a_request_with_its_id() {
        let decoded = call(r#"{"jsonrpc":"2.0","id":1,"method":"reload"}"#);
        assert_eq!(
            decoded,
            Incoming::Call {
                id: Some(json!(1)),
                method: Method::Reload,
            }
        );
    }

    #[test]
    fn a_notification_decodes_with_no_id() {
        let decoded = call(r#"{"jsonrpc":"2.0","method":"shutdown"}"#);
        assert_eq!(
            decoded,
            Incoming::Call {
                id: None,
                method: Method::Shutdown,
            }
        );
    }

    #[test]
    fn a_null_id_stays_distinct_from_an_absent_one() {
        let decoded = call(r#"{"jsonrpc":"2.0","id":null,"method":"reload"}"#);
        assert_eq!(
            decoded,
            Incoming::Call {
                id: Some(Value::Null),
                method: Method::Reload,
            }
        );
    }

    #[test]
    fn a_string_id_round_trips_unchanged() {
        let Incoming::Call { id, .. } = call(r#"{"jsonrpc":"2.0","id":"a7","method":"reload"}"#)
        else {
            panic!("expected a call");
        };
        assert_eq!(id, Some(json!("a7")));
    }

    #[test]
    fn an_unknown_method_decodes_rather_than_failing() {
        let decoded = call(r#"{"jsonrpc":"2.0","id":1,"method":"vite"}"#);
        assert_eq!(
            decoded,
            Incoming::Call {
                id: Some(json!(1)),
                method: Method::Unknown,
            }
        );
    }

    #[test]
    fn a_blank_line_decodes_as_empty() {
        assert_eq!(call(""), Incoming::Empty);
        assert_eq!(call("   \t "), Incoming::Empty);
    }

    #[test]
    fn only_broken_json_syntax_is_a_parse_error() {
        for line in [r"{", r#"{"jsonrpc":}"#, "{,}"] {
            let (code, _) = decode(line).unwrap_err();
            assert_eq!(code, ErrorCode::ParseError, "{line}");
        }
    }

    #[test]
    fn json_that_parses_but_is_no_request_is_an_invalid_request() {
        for line in [
            r#"["2.0",1,"reload"]"#,
            r#""reload""#,
            "7",
            "null",
            r#"{"id":1,"method":"reload"}"#,
            r#"{"jsonrpc":"1.0","id":1,"method":"reload"}"#,
            r#"{"jsonrpc":"2.0","id":1}"#,
            r#"{"jsonrpc":"2.0","id":{"n":1},"method":"reload"}"#,
            r#"{"jsonrpc":"2.0","id":[1],"method":"reload"}"#,
        ] {
            let (code, _) = decode(line).unwrap_err();
            assert_eq!(code, ErrorCode::InvalidRequest, "{line}");
        }
    }

    #[test]
    fn encodes_the_documented_shapes() {
        let result = encode(&Outgoing::result(json!(1))).unwrap();
        assert_eq!(result, r#"{"jsonrpc":"2.0","id":1,"result":null}"#);

        let failure = encode(&Outgoing::error(
            Value::Null,
            ErrorCode::ParseError,
            "bad line",
        ))
        .unwrap();
        assert_eq!(
            failure,
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"bad line"}}"#
        );

        let ready = encode(&Outgoing::ready()).unwrap();
        assert_eq!(
            ready,
            r#"{"jsonrpc":"2.0","method":"ready","params":{"protocol":0}}"#
        );
    }

    /// A real `EngineError`, since only the engine can produce one.
    fn thrown_by(source: &str) -> EngineError {
        let engine = inca_jsenv::Engine::new().unwrap();
        engine.eval::<()>(source).unwrap_err()
    }

    #[test]
    fn a_thrown_value_keeps_its_stack_beside_its_message() {
        let err = thrown_by("throw new Error('bad edit');");

        let encoded = encode(&Outgoing::thrown(json!(5), ErrorCode::BundleFailed, &err)).unwrap();

        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded["error"]["code"], -32000);
        assert_eq!(decoded["error"]["message"], "Error: bad edit");
        assert!(
            decoded["error"]["data"]["stack"]
                .as_str()
                .unwrap()
                .contains("at ")
        );
    }

    #[test]
    fn a_thrown_non_error_reports_a_null_stack() {
        let err = thrown_by("throw 'just a string';");

        let encoded = encode(&Outgoing::thrown(
            Value::Null,
            ErrorCode::BundleFailed,
            &err,
        ))
        .unwrap();

        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded["error"]["message"], "just a string");
        assert!(decoded["error"]["data"]["stack"].is_null());
    }

    #[test]
    fn an_app_error_answers_no_id() {
        let err = thrown_by("throw new Error('boom');");

        let encoded = encode(&Outgoing::app_error(&err)).unwrap();

        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded["method"], "appError");
        assert!(decoded.get("id").is_none(), "nothing asked for it");
        assert_eq!(decoded["params"]["message"], "Error: boom");
        assert!(decoded["params"]["stack"].is_string());
    }

    #[test]
    fn every_encoded_message_is_one_line() {
        let messages = [
            Outgoing::ready(),
            Outgoing::result(json!(1)),
            Outgoing::error(json!(1), ErrorCode::BundleFailed, "a\nb"),
        ];
        for message in messages {
            assert!(!encode(&message).unwrap().contains('\n'));
        }
    }
}
