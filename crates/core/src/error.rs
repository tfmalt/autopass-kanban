//! Typed error enum at the `kanban-core` public boundary.
//!
//! `KanbanError` is the typed error contract for the crate. Public core
//! functions surface typed errors so the JSON `code` field derives from the
//! enum variant rather than from string-sniffing `anyhow` message prose
//! (US-025). `anyhow` remains the internal error model; typed errors are
//! attached at the origin of each classified failure and recovered via
//! `anyhow::Error::downcast_ref::<KanbanError>()` at the boundary.

use std::fmt;

use crate::json::KanbanErrorCode;

/// Typed error returned from classified `kanban-core` failure paths.
///
/// Variants correspond 1:1 to [`KanbanErrorCode`] so that rewording an error
/// message can never change the machine-readable `code` clients receive.
#[derive(Debug)]
pub enum KanbanError {
    /// The target directory is not a kanban-initialized repository.
    NotInitialized,
    /// A user story id could not be resolved.
    StoryNotFound(String),
    /// A sprint name could not be resolved.
    SprintNotFound(String),
    /// An epic id could not be resolved.
    EpicNotFound(String),
    /// A phase label could not be resolved.
    PhaseNotFound(String),
    /// A story or task status is not part of the canonical workflow vocabulary.
    InvalidStatus(String),
    /// A CLI argument or frontmatter value is invalid.
    InvalidArgument(String),
    /// A feature flag is disabled in `.kanban/settings.json`.
    FeatureDisabled(String),
    /// A config key was not found.
    ConfigKeyNotFound(String),
    /// An I/O error reading or writing a backlog file.
    Io(String),
    /// A markdown/frontmatter parse error.
    ParseError(String),
    /// An uncategorized internal failure. Wraps the originating `anyhow` error
    /// so no diagnostic detail is lost.
    Internal(anyhow::Error),
}

impl KanbanError {
    pub fn not_initialized() -> Self {
        KanbanError::NotInitialized
    }

    pub fn story_not_found(id: impl Into<String>) -> Self {
        KanbanError::StoryNotFound(id.into())
    }

    pub fn sprint_not_found(name: impl Into<String>) -> Self {
        KanbanError::SprintNotFound(name.into())
    }

    pub fn epic_not_found(id: impl Into<String>) -> Self {
        KanbanError::EpicNotFound(id.into())
    }

    pub fn invalid_status(status: impl Into<String>) -> Self {
        KanbanError::InvalidStatus(status.into())
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        KanbanError::InvalidArgument(message.into())
    }

    pub fn feature_disabled(feature: impl Into<String>) -> Self {
        KanbanError::FeatureDisabled(feature.into())
    }
}

impl fmt::Display for KanbanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KanbanError::NotInitialized => {
                write!(
                    f,
                    "Repository is not initialized. Run `kanban init` to create the .kanban configuration."
                )
            }
            KanbanError::StoryNotFound(id) => write!(f, "Story not found: {id}"),
            KanbanError::SprintNotFound(name) => write!(f, "Sprint not found: {name}"),
            KanbanError::EpicNotFound(id) => write!(f, "Epic not found: {id}"),
            KanbanError::PhaseNotFound(phase) => write!(f, "Phase not found: {phase}"),
            KanbanError::InvalidStatus(status) => {
                write!(f, "Unsupported story status: {status}")
            }
            KanbanError::InvalidArgument(message) => write!(f, "{message}"),
            KanbanError::FeatureDisabled(feature) => {
                write!(
                    f,
                    "Feature '{feature}' is disabled in .kanban/settings.json. Run `kanban features enable {feature}` to re-enable it."
                )
            }
            KanbanError::ConfigKeyNotFound(key) => write!(f, "Config key not found: {key}"),
            KanbanError::Io(message) => write!(f, "I/O error: {message}"),
            KanbanError::ParseError(message) => write!(f, "Parse error: {message}"),
            KanbanError::Internal(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for KanbanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            KanbanError::Internal(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Attach a typed [`KanbanError`] to an `anyhow` chain so the boundary can
/// recover it via `downcast_ref::<KanbanError>()`. The conversion is provided
/// by `anyhow`'s blanket `impl From<E> for anyhow::Error where E: Error`; we
/// only ensure `KanbanError: std::error::Error` here.
impl From<std::io::Error> for KanbanError {
    fn from(value: std::io::Error) -> Self {
        KanbanError::Io(value.to_string())
    }
}

/// Wrap any `anyhow::Error` as `KanbanError::Internal` so public functions
/// returning `Result<_, KanbanError>` can use `?` on internal anyhow results
/// without individually mapping every error source (US-025 AC#2).
///
/// Typed `KanbanError` variants already attached to the anyhow chain are
/// recovered at the boundary by [`KanbanErrorCode::from`] looking inside
/// `Internal`.
impl From<anyhow::Error> for KanbanError {
    fn from(error: anyhow::Error) -> Self {
        // If the anyhow error already carries a typed KanbanError, unwrap it
        // instead of double-wrapping. This preserves the typed variant so the
        // boundary classification yields the correct code, not `Internal`.
        match error.downcast::<KanbanError>() {
            Ok(typed) => typed,
            Err(other) => KanbanError::Internal(other),
        }
    }
}

/// Map a typed [`KanbanError`] to the stable JSON [`KanbanErrorCode`].
///
/// This replaces the legacy string-sniffing `KanbanErrorCode::classify` for
/// errors that carry a typed payload: the code is derived from the variant,
/// not from the error message, so rewording a `bail!`/`Display` message cannot
/// silently change the code clients receive.
impl From<&KanbanError> for KanbanErrorCode {
    fn from(error: &KanbanError) -> Self {
        match error {
            KanbanError::NotInitialized => KanbanErrorCode::NotInitialized,
            KanbanError::StoryNotFound(_) => KanbanErrorCode::StoryNotFound,
            KanbanError::SprintNotFound(_) => KanbanErrorCode::SprintNotFound,
            KanbanError::EpicNotFound(_) => KanbanErrorCode::EpicNotFound,
            KanbanError::PhaseNotFound(_) => KanbanErrorCode::PhaseNotFound,
            KanbanError::InvalidStatus(_) => KanbanErrorCode::InvalidStatus,
            KanbanError::InvalidArgument(_) => KanbanErrorCode::InvalidArgument,
            KanbanError::FeatureDisabled(_) => KanbanErrorCode::NotInitialized,
            KanbanError::ConfigKeyNotFound(_) => KanbanErrorCode::ConfigKeyNotFound,
            KanbanError::Io(_) => KanbanErrorCode::IoError,
            KanbanError::ParseError(_) => KanbanErrorCode::ParseError,
            // Look inside Internal for a typed KanbanError that was wrapped by
            // the `From<anyhow::Error>` impl. If the inner error is not typed,
            // classify as Internal.
            KanbanError::Internal(inner) => {
                if let Some(typed) = inner.downcast_ref::<KanbanError>() {
                    KanbanErrorCode::from(typed)
                } else {
                    KanbanErrorCode::Internal
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_variant_maps_to_a_stable_code() {
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::NotInitialized),
            KanbanErrorCode::NotInitialized
        );
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::StoryNotFound("US-1".to_string())),
            KanbanErrorCode::StoryNotFound
        );
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::SprintNotFound("S001".to_string())),
            KanbanErrorCode::SprintNotFound
        );
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::InvalidStatus("bogus".to_string())),
            KanbanErrorCode::InvalidStatus
        );
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::FeatureDisabled("sprints".to_string())),
            KanbanErrorCode::NotInitialized
        );
        assert_eq!(
            KanbanErrorCode::from(&KanbanError::Io("boom".to_string())),
            KanbanErrorCode::IoError
        );
    }

    #[test]
    fn typed_error_survives_anyhow_roundtrip_and_classifies_without_string_sniffing() {
        let error: anyhow::Error = KanbanError::SprintNotFound("S099".to_string()).into();
        let recovered = error
            .downcast_ref::<KanbanError>()
            .expect("typed error must survive anyhow roundtrip");
        let code = KanbanErrorCode::from(recovered);

        assert_eq!(code, KanbanErrorCode::SprintNotFound);
        assert!(error.to_string().contains("S099"));
    }

    #[test]
    fn rewording_message_does_not_change_code() {
        // The code comes from the variant, so a Display wording change cannot
        // alter the code clients receive (US-003 scenario 3).
        let err = KanbanError::SprintNotFound("S001".to_string());
        let code = KanbanErrorCode::from(&err);
        assert_eq!(code, KanbanErrorCode::SprintNotFound);

        // Even if the message text differs, the variant-driven code is stable.
        let err2 = KanbanError::SprintNotFound("completely different prose".to_string());
        assert_eq!(
            KanbanErrorCode::from(&err2),
            KanbanErrorCode::SprintNotFound
        );
    }

    #[test]
    fn from_anyhow_error_unwraps_typed_kanban_error() {
        // US-025 AC#2: when a public function returns Result<_, KanbanError>
        // and uses `?` on an anyhow result carrying a typed KanbanError, the
        // From impl unwraps the typed variant instead of wrapping in Internal.
        let anyhow_err: anyhow::Error = KanbanError::StoryNotFound("US-001".to_string()).into();
        let kanban_err: KanbanError = anyhow_err.into();
        assert!(
            matches!(kanban_err, KanbanError::StoryNotFound(ref id) if id == "US-001"),
            "From<anyhow::Error> should unwrap typed KanbanError, got {kanban_err:?}"
        );
        assert_eq!(
            KanbanErrorCode::from(&kanban_err),
            KanbanErrorCode::StoryNotFound
        );
    }

    #[test]
    fn from_anyhow_error_wraps_untyped_as_internal() {
        let anyhow_err = anyhow::anyhow!("some unclassified error");
        let kanban_err: KanbanError = anyhow_err.into();
        assert!(
            matches!(kanban_err, KanbanError::Internal(_)),
            "untyped anyhow error should wrap as Internal, got {kanban_err:?}"
        );
        assert_eq!(
            KanbanErrorCode::from(&kanban_err),
            KanbanErrorCode::Internal
        );
    }

    #[test]
    fn internal_variant_classifies_typed_inner_error() {
        // If someone manually wraps a typed anyhow error in Internal, the
        // classifier should still find the typed variant inside.
        let typed: anyhow::Error = KanbanError::SprintNotFound("S099".to_string()).into();
        let wrapped = KanbanError::Internal(typed);
        assert_eq!(
            KanbanErrorCode::from(&wrapped),
            KanbanErrorCode::SprintNotFound,
            "Internal variant should look inside for typed error"
        );
    }
}
