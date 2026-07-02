use serde::Serialize;

use super::SCHEMA_VERSION;

/// Top-level status of a JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Ok,
    Warning,
    Error,
}

/// Machine-readable error code embedded in `KanbanErrorBody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KanbanErrorCode {
    NotInitialized,
    StoryNotFound,
    SprintNotFound,
    EpicNotFound,
    PhaseNotFound,
    InvalidStatus,
    InvalidArgument,
    ConfigKeyNotFound,
    IoError,
    ParseError,
    Internal,
}

/// Error body embedded in a JSON envelope when `status` is `"error"`.
#[derive(Debug, Clone, Serialize)]
pub struct KanbanErrorBody {
    pub code: KanbanErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl KanbanErrorBody {
    pub fn new(code: KanbanErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn from_anyhow(error: &anyhow::Error) -> Self {
        if let Some(typed) = error.downcast_ref::<crate::error::KanbanError>() {
            return Self::new(KanbanErrorCode::from(typed), error.to_string());
        }
        Self::new(KanbanErrorCode::Internal, error.to_string())
    }
}

/// Top-level JSON envelope emitted by `--format json`.
#[derive(Debug, Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub status: ResultStatus,
    pub kind: &'static str,
    pub schema_version: u32,
    pub data: Option<T>,
    pub error: Option<KanbanErrorBody>,
}

impl<T: Serialize> JsonEnvelope<T> {
    pub fn ok(kind: &'static str, data: T) -> Self {
        Self {
            status: ResultStatus::Ok,
            kind,
            schema_version: SCHEMA_VERSION,
            data: Some(data),
            error: None,
        }
    }

    pub fn warning(kind: &'static str, data: T) -> Self {
        Self {
            status: ResultStatus::Warning,
            kind,
            schema_version: SCHEMA_VERSION,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(kind: &'static str, body: KanbanErrorBody) -> Self {
        Self {
            status: ResultStatus::Error,
            kind,
            schema_version: SCHEMA_VERSION,
            data: None,
            error: Some(body),
        }
    }

    /// Returns the process exit code for this envelope.
    pub fn exit_code(&self) -> i32 {
        match self.status {
            ResultStatus::Ok => 0,
            ResultStatus::Warning => 2,
            ResultStatus::Error => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigGetDto;

    #[test]
    fn ok_envelope_serializes_with_all_keys() {
        let env = JsonEnvelope::ok(
            "config.get",
            ConfigGetDto {
                key: "paths.backlog".to_string(),
                value: "delivery/backlog".to_string(),
            },
        );
        let json = serde_json::to_value(&env).expect("serialization should succeed");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["kind"], "config.get");
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["data"]["key"], "paths.backlog");
        assert_eq!(json["data"]["value"], "delivery/backlog");
        assert!(json["error"].is_null());
    }

    #[test]
    fn error_envelope_has_null_data_and_populated_error() {
        let env: JsonEnvelope<ConfigGetDto> = JsonEnvelope::error(
            "config.get",
            KanbanErrorBody::new(KanbanErrorCode::ConfigKeyNotFound, "no such key"),
        );
        let json = serde_json::to_value(&env).expect("serialization should succeed");
        assert_eq!(json["status"], "error");
        assert!(json["data"].is_null());
        assert_eq!(json["error"]["code"], "config_key_not_found");
        assert_eq!(json["error"]["message"], "no such key");
        assert!(json["error"]["details"].is_null());
    }

    #[test]
    fn error_code_serializes_as_snake_case() {
        let value = serde_json::to_value(KanbanErrorCode::StoryNotFound)
            .expect("serialization should succeed");
        assert_eq!(
            value,
            serde_json::Value::String("story_not_found".to_string())
        );
    }

    #[test]
    fn from_anyhow_prefers_typed_kanban_error() {
        let typed: anyhow::Error = crate::error::KanbanError::sprint_not_found("S099").into();
        let body = KanbanErrorBody::from_anyhow(&typed);
        assert_eq!(body.code, KanbanErrorCode::SprintNotFound);
        assert!(body.message.contains("S099"));
    }

    #[test]
    fn from_anyhow_legacy_fallback_is_internal() {
        let plain = anyhow::anyhow!("Story not found: US-1");
        let body = KanbanErrorBody::from_anyhow(&plain);
        assert_eq!(body.code, KanbanErrorCode::Internal);
    }
}
