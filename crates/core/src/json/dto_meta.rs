use serde::Serialize;

use crate::{CompletionItem, ConfigInitResult, ConfigSetResult};

use super::{non_empty, path_string};

/// Placeholder data type for error-only envelopes where the command has no DTO.
#[derive(Debug, Clone, Serialize)]
pub struct NoData;

/// DTO for `config get` responses.
#[derive(Debug, Serialize)]
pub struct ConfigGetDto {
    pub key: String,
    pub value: String,
}

/// DTO for `init` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigInitDto {
    pub repo_root: String,
    pub config_dir: String,
    pub created_files: Vec<String>,
    pub created_count: usize,
}

impl ConfigInitDto {
    pub fn from_result(r: &ConfigInitResult) -> Self {
        let created_files: Vec<String> = r.created_files.iter().map(|p| path_string(p)).collect();
        let created_count = created_files.len();
        Self {
            repo_root: path_string(&r.repo_root),
            config_dir: path_string(&r.config_dir),
            created_files,
            created_count,
        }
    }
}

/// DTO for `config set` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSetDto {
    pub key: String,
    pub value: String,
    pub file_path: String,
}

impl ConfigSetDto {
    pub fn from_result(r: &ConfigSetResult) -> Self {
        Self {
            key: r.key.clone(),
            value: r.value.clone(),
            file_path: path_string(&r.file_path),
        }
    }
}

/// DTO for `completion` responses in JSON mode.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionDto {
    pub target: String,
    pub content_type: String,
    pub content: String,
}

/// DTO item for `list-ids` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ListIdItemDto {
    pub value: String,
    pub description: Option<String>,
}

impl ListIdItemDto {
    pub fn value(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: None,
        }
    }

    pub fn from_completion_item(item: &CompletionItem) -> Self {
        Self {
            value: item.value.clone(),
            description: non_empty(&item.description),
        }
    }
}

/// DTO for hidden `list-ids` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ListIdsDto {
    pub kind: String,
    pub count: usize,
    pub items: Vec<ListIdItemDto>,
}

impl ListIdsDto {
    pub fn new(kind: impl Into<String>, items: Vec<ListIdItemDto>) -> Self {
        let count = items.len();
        Self {
            kind: kind.into(),
            count,
            items,
        }
    }
}

/// Parse a raw config JSON string into a `serde_json::Value`.
pub fn config_show_value(config_json: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(config_json)
}
