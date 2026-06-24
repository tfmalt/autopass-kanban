use std::convert::Infallible;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use chrono::{Days, Local};
use futures::Stream;
use include_dir::{Dir, include_dir};
use kanban_core::*;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tokio::task;

use crate::AppState;
use crate::dto::*;
use crate::metrics::*;
use crate::snapshot::{load_epic_detail, load_repository_snapshot, load_story_detail};
use crate::sprint_io::{
    CreateSprintInputWeb, UpdateSprintInput, parse_date_or, update_sprint_file,
};
use crate::team::load_team;

static WEB_ASSETS: Dir<'_> = include_dir!("$KANBAN_WEB_ASSET_DIR");

#[derive(Debug)]
pub(crate) struct ApiResponse {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiResponse {
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiError {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiResponse {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: error.to_string(),
        }
    }
}

async fn run_blocking<T, F>(operation: F) -> Result<T, ApiResponse>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    task::spawn_blocking(operation)
        .await
        .map_err(|err| ApiResponse::from(anyhow::anyhow!("blocking task join failure: {err}")))?
        .map_err(ApiResponse::from)
}

async fn cached_git_branch(state: &AppState) -> Result<String, ApiResponse> {
    if let Some(branch) = state.branch_cache.lock().await.clone() {
        return Ok(branch);
    }
    let repo_root = state.repo_root.clone();
    let branch = run_blocking(move || Ok(git_branch(&repo_root))).await?;
    let mut cache = state.branch_cache.lock().await;
    if cache.is_none() {
        *cache = Some(branch.clone());
    }
    Ok(cache.clone().unwrap_or(branch))
}

pub(crate) async fn api_repository(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RepositorySnapshot>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    Ok(Json(
        run_blocking(move || load_repository_snapshot(&repo_root)).await?,
    ))
}

pub(crate) async fn api_metrics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DashboardMetrics>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    let repo = run_blocking(move || load_repository_snapshot(&repo_root)).await?;
    Ok(Json(compute_metrics(&repo)))
}

pub(crate) async fn api_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ConfigResponse>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    let config = run_blocking(move || load_kanban_config(&repo_root)).await?;
    let branch = cached_git_branch(&state).await?;
    Ok(Json(ConfigResponse {
        port: state.port,
        host: state.host.clone(),
        style: config.web.style,
        version: env!("CARGO_PKG_VERSION").to_string(),
        branch,
        story_points: StoryPointsResponse {
            allowed_values: config.story_points.allowed_values,
            aliases: config.story_points.aliases,
        },
    }))
}

pub(crate) async fn api_team(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<WebTeamMember>>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    Ok(Json(run_blocking(move || load_team(&repo_root)).await?))
}

pub(crate) async fn api_team_avatar(
    State(state): State<Arc<AppState>>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, ApiResponse> {
    if path.contains("..") || path.starts_with('/') || path.contains("\\") {
        return Err(ApiResponse::not_found("invalid path"));
    }
    let avatars_dir = state.repo_root.join(".kanban").join("team_avatars");
    let file_path = avatars_dir.join(&path);

    let canonical = file_path
        .canonicalize()
        .map_err(|_| ApiResponse::not_found("not found"))?;
    if !canonical.starts_with(&avatars_dir) {
        return Err(ApiResponse::not_found("invalid path"));
    }
    if !canonical.is_file() {
        return Err(ApiResponse::not_found("not found"));
    }

    let data = fs::read(&canonical).map_err(|_| ApiResponse::not_found("not found"))?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mut response = Body::from(data).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    Ok(response)
}

pub(crate) async fn api_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<WebStoryDetail>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    let id_for_lookup = id.clone();
    let detail = run_blocking(move || load_story_detail(&repo_root, &id_for_lookup)).await?;
    let (story, body) = detail.ok_or_else(|| ApiResponse::not_found("not found"))?;
    Ok(Json(WebStoryDetail { story, body }))
}

pub(crate) async fn api_epic(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<WebEpicDetail>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    let id_for_lookup = id.clone();
    let detail = run_blocking(move || load_epic_detail(&repo_root, &id_for_lookup)).await?;
    let (epic, body) = detail.ok_or_else(|| ApiResponse::not_found("not found"))?;
    Ok(Json(WebEpicDetail { epic, body }))
}

pub(crate) async fn api_move_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<MoveInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let id_for_move = id.clone();
    let status = input.status.clone();
    let assignee = input.assignee.clone();
    let result = run_blocking(move || {
        move_story_to_status_with_assignee(&repo_root, &id_for_move, &status, assignee.as_deref())
    })
    .await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": MoveStoryDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_plan_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<PlanInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let id_for_plan = id.clone();
    let sprint = input.sprint.clone();
    let result =
        run_blocking(move || plan_story_into_sprint(&repo_root, &id_for_plan, &sprint)).await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": PlanStoryDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_update_task(
    State(state): State<Arc<AppState>>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
    Json(input): Json<UpdateTaskInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let tags = input.tags.as_deref().map(parse_tags).unwrap_or_default();
    let repo_root = state.repo_root.clone();
    let id_for_update = id.clone();
    let task_id_for_update = task_id.clone();
    let status = input.status.clone();
    let title = input.title.clone();
    let description = input.description.clone();
    let tags_owned = if input.tags.is_some() {
        Some(tags.clone())
    } else {
        None
    };
    let result = run_blocking(move || {
        update_task_in_story(
            &repo_root,
            &id_for_update,
            &task_id_for_update,
            status.as_deref(),
            title.as_deref(),
            tags_owned.as_deref(),
            description.as_deref(),
        )
    })
    .await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": TaskMutationDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_update_story_body(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateBodyInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let id_for_lookup = id.clone();
    let body = input.body.clone();
    let source = run_blocking(move || find_story_with_source(&repo_root, &id_for_lookup)).await?;
    let (_, source) = source.ok_or_else(|| ApiResponse::not_found("not found"))?;
    run_blocking(move || {
        let updated = replace_markdown_body(&source.markdown, &body);
        atomic_write(&source.file_path, &updated)
            .with_context(|| format!("write story file {}", source.file_path.display()))
    })
    .await?;
    let _ = state.events.send(());
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn api_update_story_fields(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateStoryFieldsInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let mut updates = Vec::new();
    if let Some(value) = input.assignee {
        updates.push(("assignee".to_string(), value));
    }
    if let Some(value) = input.sprint {
        updates.push(("sprint".to_string(), value));
    }
    if let Some(value) = input.status {
        updates.push(("status".to_string(), value));
    }
    if let Some(value) = input.story_points {
        updates.push(("story_points".to_string(), json_value_to_string(value)));
    }
    if let Some(value) = input.priority {
        updates.push(("priority".to_string(), value.to_string()));
    }
    if updates.is_empty() {
        return Err(ApiResponse::bad_request(
            "at least one field must be provided",
        ));
    }
    let repo_root = state.repo_root.clone();
    let id_for_update = id.clone();
    let result =
        run_blocking(move || update_story_frontmatter(&repo_root, &id_for_update, &updates))
            .await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": StoryUpdateDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_update_epic_fields(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateEpicFieldsInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let updates = [("priority".to_string(), input.priority.to_string())];
    let repo_root = state.repo_root.clone();
    let id_for_update = id.clone();
    let result =
        run_blocking(move || update_epic_frontmatter(&repo_root, &id_for_update, &updates)).await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": EpicUpdateDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_create_sprint(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateSprintInputWeb>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let result = run_blocking(move || {
        let number = input
            .number
            .unwrap_or(suggested_next_sprint_number(&repo_root)?);
        let (default_start, default_end) =
            suggested_next_sprint_dates(&repo_root)?.unwrap_or_else(|| {
                let start = Local::now().date_naive();
                (start, start + Days::new(11))
            });
        let start_date = parse_date_or(input.start.as_deref(), default_start)?;
        let end_date = parse_date_or(input.end.as_deref(), default_end)?;
        let create_input = kanban_core::CreateSprintInput {
            number,
            start_date,
            end_date,
            headline: input.headline,
        };
        create_sprint(&repo_root, &create_input)
    })
    .await?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": SprintCreateDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_update_sprint(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<UpdateSprintInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let name_for_update = name.clone();
    let result =
        run_blocking(move || update_sprint_file(&repo_root, &name_for_update, input)).await?;
    let _ = state.events.send(());
    Ok(Json(json!({ "ok": true, "data": result })))
}

pub(crate) async fn api_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(()) => return Some((Ok(Event::default().event("change").data("{}")), rx)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub(crate) async fn static_asset(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        path = "index.html";
    }
    let file = WEB_ASSETS
        .get_file(path)
        .or_else(|| WEB_ASSETS.get_file("index.html"));
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
            let mut response = Body::from(file.contents().to_vec()).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime.as_ref())
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            response
        }
        None => (StatusCode::NOT_FOUND, "kanban web assets are not embedded").into_response(),
    }
}

pub(crate) fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn git_branch(repo_root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn replace_markdown_body(markdown: &str, body: &str) -> String {
    if let Some(rest) = markdown.strip_prefix("---") {
        let newline = if rest.starts_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let close = format!("{newline}---");
        if let Some(end) = markdown[3..].find(&close) {
            let body_start = 3 + end + close.len();
            let prefix = &markdown[..body_start];
            return format!("{}{}{}", prefix, newline.repeat(2), body.trim_start());
        }
    }
    body.to_string()
}

pub(crate) fn json_value_to_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{Mutex, broadcast};

    #[test]
    fn replace_markdown_body_preserves_frontmatter() {
        let markdown = "---\nid: US-F1-001\n---\n# Old\n";
        let updated = replace_markdown_body(markdown, "# New\n");
        assert!(updated.starts_with("---\nid: US-F1-001\n---\n\n"));
        assert!(updated.ends_with("# New\n"));
    }

    #[tokio::test]
    async fn cached_git_branch_returns_cached_value_without_repo_access() {
        let (events, _) = broadcast::channel(8);
        let state = crate::AppState {
            repo_root: std::path::PathBuf::from("/tmp/does-not-need-to-exist"),
            host: "127.0.0.1".to_string(),
            port: 8080,
            branch_cache: Arc::new(Mutex::new(Some("cached-branch".to_string()))),
            events,
            write_lock: Arc::new(Mutex::new(())),
        };
        let branch = cached_git_branch(&state)
            .await
            .expect("cached branch should resolve");
        assert_eq!(branch, "cached-branch");
    }
}
