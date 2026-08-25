use std::convert::Infallible;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use chrono::{Days, Local};
use kanban_core::*;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tokio::task;

use crate::AppState;
use crate::changes::ChangeBroadcaster;
use crate::dto::*;
use crate::git;
use crate::metrics::*;
use crate::pending;
use crate::read_model::WebReadModel;
use crate::snapshot::{load_epic_detail, load_repository_snapshot, load_story_detail};
use crate::sprint_io::{
    CreateSprintInputWeb, UpdateSprintInput, parse_date_or, update_sprint_file,
};
use crate::team::load_team;

const SSE_SUBSCRIBER_CAP: usize = 64;

struct SseSubscriberGuard {
    count: Arc<AtomicUsize>,
}

impl Drop for SseSubscriberGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

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
        eprintln!("kanban-web internal error: {error:#}");
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "internal error".to_string(),
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
    let branch = run_blocking(move || Ok(git::branch(&repo_root))).await?;
    let mut cache = state.branch_cache.lock().await;
    if cache.is_none() {
        *cache = Some(branch.clone());
    }
    Ok(cache.clone().unwrap_or(branch))
}

async fn record_change(state: &AppState, summary: String, paths: Vec<PathBuf>) {
    let Some(store) = state.pending.clone() else {
        return;
    };
    if let Err(error) = run_blocking(move || store.record(summary, paths)).await {
        eprintln!(
            "kanban-web could not record pending web change: {}",
            error.message
        );
    }
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
    let metrics = run_blocking(move || Ok(WebReadModel::build(&repo_root)?.metrics())).await?;
    Ok(Json(metrics))
}

pub(crate) async fn api_report(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WebReportDashboard>, ApiResponse> {
    let repo_root = state.repo_root.clone();
    let report = run_blocking(move || Ok(WebReadModel::build(&repo_root)?.report())).await?;
    Ok(Json(report))
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

    // US-023: canonicalize + read are blocking filesystem calls and must not run
    // on the async runtime.
    let data = run_blocking(move || {
        let canonical = file_path
            .canonicalize()
            .with_context(|| format!("resolve avatar {}", file_path.display()))?;
        if !canonical.starts_with(&avatars_dir) || !canonical.is_file() {
            anyhow::bail!("avatar path is outside the avatars directory");
        }
        fs::read(&canonical).with_context(|| format!("read avatar {}", canonical.display()))
    })
    .await
    .map_err(|_| ApiResponse::not_found("not found"))?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    if mime.type_().as_str() != "image" {
        let mut response = ApiResponse::not_found("not found").into_response();
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        return Ok(response);
    }
    let mut response = Body::from(data).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
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
    let paths = std::iter::once(result.story_path.clone())
        .chain(result.task_path.clone())
        .collect();
    record_change(
        &state,
        format!(
            "{} moved {} to {}",
            id, result.from_status, result.to_status
        ),
        paths,
    )
    .await;
    state.changes.notify();
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
    let paths = std::iter::once(result.story_path.clone())
        .chain(result.task_path.clone())
        .collect();
    record_change(
        &state,
        format!("{} planned into {}", id, result.sprint_name),
        paths,
    )
    .await;
    state.changes.notify();
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
    record_change(
        &state,
        format!("{id}/{task_id} updated"),
        vec![result.task_file_path.clone()],
    )
    .await;
    state.changes.notify();
    Ok(Json(
        json!({ "ok": true, "data": TaskMutationDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_create_task(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<CreateTaskInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let tags = input.tags.as_deref().map(parse_tags).unwrap_or_default();
    let repo_root = state.repo_root.clone();
    let id_for_create = id.clone();
    let title = input.title;
    let status = input.status.unwrap_or_else(|| "todo".to_string());
    let description = input.description.unwrap_or_default();
    let result = run_blocking(move || {
        add_task_to_story(
            &repo_root,
            &id_for_create,
            &title,
            &status,
            &tags,
            &description,
        )
    })
    .await?;
    record_change(
        &state,
        format!("{id}/{} created", result.task_id),
        vec![result.task_file_path.clone()],
    )
    .await;
    state.changes.notify();
    Ok(Json(
        json!({ "ok": true, "data": TaskMutationDto::from_result(&result, &state.repo_root) }),
    ))
}

pub(crate) async fn api_delete_task(
    State(state): State<Arc<AppState>>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let id_for_delete = id.clone();
    let task_id_for_delete = task_id.clone();
    let result = run_blocking(move || {
        delete_task_from_story(&repo_root, &id_for_delete, &task_id_for_delete)
    })
    .await?;
    record_change(
        &state,
        format!("{id}/{task_id} deleted"),
        vec![result.task_file_path.clone()],
    )
    .await;
    state.changes.notify();
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn api_reorder_tasks(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<ReorderTasksInput>,
) -> Result<Json<Value>, ApiResponse> {
    let _write_guard = state.write_lock.lock().await;
    let repo_root = state.repo_root.clone();
    let id_for_reorder = id.clone();
    let result =
        run_blocking(move || reorder_tasks_in_story(&repo_root, &id_for_reorder, &input.task_ids))
            .await?;
    record_change(
        &state,
        format!("{id} tasks reordered"),
        vec![result.task_file_path.clone()],
    )
    .await;
    state.changes.notify();
    Ok(Json(json!({ "ok": true })))
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
    let source_path = source.file_path.clone();
    run_blocking(move || {
        let updated = replace_markdown_body(&source.markdown, &body);
        atomic_write(&source.file_path, &updated)
            .with_context(|| format!("write story file {}", source.file_path.display()))
    })
    .await?;
    record_change(
        &state,
        format!("{id} description updated"),
        vec![source_path],
    )
    .await;
    state.changes.notify();
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
    record_change(
        &state,
        format!("{} updated {}", id, result.updated_fields.join(", ")),
        vec![result.story_path.clone()],
    )
    .await;
    state.changes.notify();
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
    let mut updates = Vec::new();
    if let Some(value) = input.priority {
        updates.push(("priority".to_string(), value.to_string()));
    }
    if let Some(value) = input.planned_start {
        updates.push(("planned_start".to_string(), value));
    }
    if let Some(value) = input.planned_end {
        updates.push(("planned_end".to_string(), value));
    }
    if let Some(value) = input.work_started {
        updates.push(("work_started".to_string(), value));
    }
    if let Some(value) = input.work_done {
        updates.push(("work_done".to_string(), value));
    }
    if updates.is_empty() {
        return Err(ApiResponse::bad_request(
            "at least one field must be provided",
        ));
    }
    let repo_root = state.repo_root.clone();
    let id_for_update = id.clone();
    let result =
        run_blocking(move || update_epic_frontmatter(&repo_root, &id_for_update, &updates)).await?;
    record_change(
        &state,
        format!("{} updated {}", id, result.updated_fields.join(", ")),
        vec![result.epic_path.clone()],
    )
    .await;
    state.changes.notify();
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
    record_change(
        &state,
        format!("sprint {} created", result.sprint_name),
        vec![result.sprint_path.clone()],
    )
    .await;
    state.changes.notify();
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
    let path = result
        .get("sprintPath")
        .and_then(Value::as_str)
        .map(|path| state.repo_root.join(path))
        .into_iter()
        .collect();
    record_change(&state, format!("sprint {name} updated"), path).await;
    state.changes.notify();
    Ok(Json(json!({ "ok": true, "data": result })))
}

/// State carried through the SSE stream.
///
/// The subscriber guard lives here rather than in `api_events`'s body: a local
/// `_guard` would be dropped the moment the handler returned the response, so
/// the subscriber count would fall back to zero while the stream was still
/// open and `SSE_SUBSCRIBER_CAP` would never actually bound anything.
struct SseStream {
    rx: broadcast::Receiver<u64>,
    changes: ChangeBroadcaster,
    shutdown: tokio::sync::watch::Receiver<bool>,
    /// Emitted before the first live event when the client is known to have
    /// missed changes (reconnect gap or a lagged receiver).
    pending_resync: Option<u64>,
    _guard: SseSubscriberGuard,
}

fn change_event(generation: u64, reason: &str) -> Event {
    Event::default()
        .id(generation.to_string())
        .event("change")
        .data(json!({ "generation": generation, "reason": reason }).to_string())
}

pub(crate) async fn api_events(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let previous = state.sse_subscribers.fetch_add(1, Ordering::SeqCst);
    if previous >= SSE_SUBSCRIBER_CAP {
        state.sse_subscribers.fetch_sub(1, Ordering::SeqCst);
        return (StatusCode::SERVICE_UNAVAILABLE, "too many SSE subscribers").into_response();
    }
    let guard = SseSubscriberGuard {
        count: state.sse_subscribers.clone(),
    };

    // Subscribe before reading the current generation so no change can slip
    // between the two and be missed by both the resync check and the stream.
    let rx = state.changes.subscribe();
    let current = state.changes.current_generation();

    // `EventSource` replays the last seen id on reconnect. If the client is
    // behind, it missed every change during the gap; tell it to resynchronize
    // immediately instead of waiting for the next unrelated edit.
    let pending_resync = last_event_id(&headers)
        .filter(|seen| *seen < current)
        .map(|_| current);

    let stream = futures::stream::unfold(
        SseStream {
            rx,
            changes: state.changes.clone(),
            shutdown: state.shutdown.clone(),
            pending_resync,
            _guard: guard,
        },
        |mut state| async move {
            if let Some(generation) = state.pending_resync.take() {
                return Some((
                    Ok::<Event, Infallible>(change_event(generation, "resync")),
                    state,
                ));
            }
            if *state.shutdown.borrow() {
                return None;
            }
            let next = tokio::select! {
                // End the stream on shutdown so the connection closes and the
                // server's graceful shutdown can actually complete.
                _ = state.shutdown.changed() => return None,
                next = state.rx.recv() => next,
            };
            match next {
                Ok(generation) => Some((Ok(change_event(generation, "change")), state)),
                // The subscriber fell behind the broadcast buffer. Silently
                // continuing would drop those changes permanently, so tell the
                // client to refetch at the current generation instead.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let generation = state.changes.current_generation();
                    Some((Ok(change_event(generation, "resync")), state))
                }
                Err(broadcast::error::RecvError::Closed) => None,
            }
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn last_event_id(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

pub(crate) async fn static_asset(uri: Uri, headers: HeaderMap) -> Response {
    crate::static_assets::serve(&uri, &headers)
}

const GIT_PULL_TIMEOUT_SECS: u64 = 60;

pub(crate) async fn api_git_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GitStatusResponse>, ApiResponse> {
    let Some(git_context) = state.git.clone() else {
        return Ok(Json(GitStatusResponse {
            available: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            pending_count: 0,
        }));
    };
    let pending = state.pending.clone();
    let status = run_blocking(move || {
        // Refresh the upstream tracking ref so the polled pull count reflects
        // remote changes, not only the last time a user manually pulled.
        let _ = git::run(
            &git_context.repo_root,
            &["fetch", "--quiet", "--no-tags"],
            true,
        );
        let upstream = git::upstream_state(&git_context.repo_root);
        let pending_count = pending
            .map(|store| store.load().map(|changes| changes.len()))
            .transpose()?
            .unwrap_or(0);
        Ok(GitStatusResponse {
            available: true,
            upstream: upstream.upstream,
            ahead: upstream.ahead,
            behind: upstream.behind,
            pending_count,
        })
    })
    .await?;
    Ok(Json(status))
}

pub(crate) async fn api_git_push(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GitPushResponse>, ApiResponse> {
    if state
        .push_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(Json(GitPushResponse {
            ok: false,
            status: "in_progress",
            message: "A push is already in progress.".to_string(),
            commit_sha: None,
        }));
    }
    struct ResetFlag(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for ResetFlag {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _reset = ResetFlag(state.push_in_progress.clone());
    let Some(git_context) = state.git.clone() else {
        return Ok(Json(GitPushResponse {
            ok: false,
            status: "error",
            message: "The data directory is not a git repository.".to_string(),
            commit_sha: None,
        }));
    };
    let Some(pending_store) = state.pending.clone() else {
        return Ok(Json(GitPushResponse {
            ok: false,
            status: "error",
            message: "Pending web changes are unavailable.".to_string(),
            commit_sha: None,
        }));
    };
    let _write_guard = state.write_lock.lock().await;
    let result = run_blocking(move || {
        let changes = pending_store.load()?;
        let upstream = git::upstream_state(&git_context.repo_root);
        if let Some(message) = git::push_preflight_error(&upstream) {
            return Ok(GitPushResponse {
                ok: false,
                status: "error",
                message: message.to_string(),
                commit_sha: None,
            });
        }
        if changes.is_empty() && upstream.ahead == 0 {
            return Ok(GitPushResponse {
                ok: true,
                status: "nothing_to_do",
                message: "Nothing to commit or push.".to_string(),
                commit_sha: None,
            });
        }
        let mut commit_sha = None;
        if !changes.is_empty() {
            let paths = pending::commit_paths(&changes)?;
            let existing_paths = paths
                .iter()
                .filter(|path| {
                    git::run(
                        &git_context.repo_root,
                        &["ls-files", "--error-unmatch", "--", path],
                        false,
                    )
                    .is_ok_and(|output| output.status.success())
                })
                .cloned()
                .collect::<Vec<_>>();
            let new_paths = paths
                .iter()
                .filter(|path| {
                    !existing_paths.contains(path) && git_context.repo_root.join(path).exists()
                })
                .cloned()
                .collect::<Vec<_>>();
            let commit_paths = existing_paths
                .iter()
                .chain(&new_paths)
                .cloned()
                .collect::<Vec<_>>();
            if !new_paths.is_empty() {
                let mut args = vec![
                    "add".to_string(),
                    "--intent-to-add".to_string(),
                    "--".to_string(),
                ];
                args.extend(new_paths);
                let output = git::run_owned(&git_context.repo_root, &args, false)?;
                if !output.status.success() {
                    return Ok(GitPushResponse {
                        ok: false,
                        status: "error",
                        message: format!(
                            "Commit preparation failed: {}",
                            git::output_text(&output)
                        ),
                        commit_sha: None,
                    });
                }
            }
            if !commit_paths.is_empty() {
                let mut args = vec![
                    "commit".to_string(),
                    "--only".to_string(),
                    "-m".to_string(),
                    pending::commit_message(&changes),
                    "--".to_string(),
                ];
                args.extend(commit_paths);
                let output = git::run_owned(&git_context.repo_root, &args, false)?;
                if !output.status.success() {
                    return Ok(GitPushResponse {
                        ok: false,
                        status: "error",
                        message: format!(
                            "Commit failed: {}",
                            git::output_text(&output)
                                .chars()
                                .take(200)
                                .collect::<String>()
                        ),
                        commit_sha: None,
                    });
                }
                let head = git::run(&git_context.repo_root, &["rev-parse", "HEAD"], false)?;
                commit_sha = Some(String::from_utf8_lossy(&head.stdout).trim().to_string());
                // A failed push leaves the commit ahead of its upstream, but
                // the ledger is safe to clear once its entries are committed.
                pending_store.clear()?;
            }
        }
        let output = git::run(&git_context.repo_root, &["push"], true)?;
        if output.status.success() {
            Ok(GitPushResponse {
                ok: true,
                status: "success",
                message: "Changes pushed.".to_string(),
                commit_sha,
            })
        } else {
            Ok(GitPushResponse {
                ok: false,
                status: "error",
                message: git::classify_push_error(&git::output_text(&output)),
                commit_sha,
            })
        }
    })
    .await?;
    if result.ok {
        state.changes.notify();
    }
    Ok(Json(result))
}

pub(crate) async fn api_git_pull(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GitPullResponse>, ApiResponse> {
    // Prevent concurrent pulls
    let was_running = state
        .pull_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err();
    if was_running {
        return Ok(Json(GitPullResponse {
            ok: false,
            status: "in_progress",
            message: "A sync is already in progress.".to_string(),
            stdout: None,
            stderr: None,
            pulled_at: None,
        }));
    }
    let _write_guard = state.write_lock.lock().await;

    let repo_root = state.repo_root.clone();
    let pull_flag = state.pull_in_progress.clone();

    let result = tokio::time::timeout(
        Duration::from_secs(GIT_PULL_TIMEOUT_SECS),
        task::spawn_blocking(move || git::run(&repo_root, &["pull", "--ff-only"], true)),
    )
    .await;

    pull_flag.store(false, Ordering::SeqCst);

    match result {
        Err(_elapsed) => Ok(Json(GitPullResponse {
            ok: false,
            status: "error",
            message: format!(
                "git pull timed out after {} seconds.",
                GIT_PULL_TIMEOUT_SECS
            ),
            stdout: None,
            stderr: None,
            pulled_at: None,
        })),
        Ok(Err(join_err)) => {
            eprintln!("kanban git-pull task join error: {join_err}");
            Ok(Json(GitPullResponse {
                ok: false,
                status: "error",
                message: "Internal error running git pull.".to_string(),
                stdout: None,
                stderr: None,
                pulled_at: None,
            }))
        }
        Ok(Ok(Err(io_err))) => {
            let message = if io_err.kind() == std::io::ErrorKind::NotFound {
                "git executable not found. Ensure git is installed and on PATH.".to_string()
            } else {
                format!("Failed to run git: {io_err}")
            };
            Ok(Json(GitPullResponse {
                ok: false,
                status: "error",
                message,
                stdout: None,
                stderr: None,
                pulled_at: None,
            }))
        }
        Ok(Ok(Ok(output))) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if output.status.success() {
                state.changes.notify();
                Ok(Json(GitPullResponse {
                    ok: true,
                    status: "success",
                    message: stdout.trim().to_string(),
                    stdout: Some(stdout),
                    stderr: if stderr.trim().is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                    pulled_at: Some(Local::now().to_rfc3339()),
                }))
            } else {
                let combined = format!("{}\n{}", stdout.trim(), stderr.trim())
                    .trim()
                    .to_string();
                let message = git::classify_pull_error(&combined);
                Ok(Json(GitPullResponse {
                    ok: false,
                    status: "error",
                    message,
                    stdout: if stdout.trim().is_empty() {
                        None
                    } else {
                        Some(stdout)
                    },
                    stderr: if stderr.trim().is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                    pulled_at: None,
                }))
            }
        }
    }
}

pub(crate) fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
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
    use axum::http::StatusCode;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn replace_markdown_body_preserves_frontmatter() {
        let markdown = "---\nid: US-F1-001\n---\n# Old\n";
        let updated = replace_markdown_body(markdown, "# New\n");
        assert!(updated.starts_with("---\nid: US-F1-001\n---\n\n"));
        assert!(updated.ends_with("# New\n"));
    }

    #[test]
    fn propagated_anyhow_error_response_hides_absolute_paths() {
        let leaked =
            anyhow::anyhow!("read story file /Users/tm/src/vegvesen/autopass-kanban/secret.md");
        let response = ApiResponse::from(leaked);
        assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(response.message, "internal error");
        assert!(!response.message.contains("/Users/"));
        assert!(!response.message.contains("autopass-kanban"));
    }

    #[test]
    fn explicit_not_found_message_is_preserved() {
        let response = ApiResponse::not_found("story not found");
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(response.message, "story not found");
    }

    #[test]
    fn avatar_non_image_response_uses_nosniff() {
        let mut response = ApiResponse::not_found("not found").into_response();
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        assert_eq!(
            response.headers().get(header::X_CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff"))
        );
    }

    #[tokio::test]
    async fn cached_git_branch_returns_cached_value_without_repo_access() {
        let mut state =
            crate::AppState::for_test(std::path::PathBuf::from("/tmp/does-not-need-to-exist"));
        state.branch_cache = Arc::new(Mutex::new(Some("cached-branch".to_string())));
        let branch = cached_git_branch(&state)
            .await
            .expect("cached branch should resolve");
        assert_eq!(branch, "cached-branch");
    }

    /// Every emitted SSE frame must carry the generation as its event id, so a
    /// reconnecting `EventSource` can tell the server where it left off.
    #[test]
    fn change_events_carry_the_generation_as_the_event_id() {
        let rendered = format!("{:?}", change_event(7, "change"));
        assert!(
            rendered.contains('7'),
            "event id must be present: {rendered}"
        );
    }

    #[test]
    fn last_event_id_parses_only_well_formed_generations() {
        let mut headers = HeaderMap::new();
        assert_eq!(last_event_id(&headers), None);
        headers.insert("last-event-id", HeaderValue::from_static(" 42 "));
        assert_eq!(last_event_id(&headers), Some(42));
        headers.insert("last-event-id", HeaderValue::from_static("not-a-number"));
        assert_eq!(last_event_id(&headers), None);
    }

    /// A client that reconnects behind the current generation missed every
    /// change in the gap, so the stream must open with a resync rather than
    /// waiting for the next unrelated edit.
    #[tokio::test(start_paused = true)]
    async fn reconnect_behind_the_current_generation_receives_an_immediate_resync() {
        use futures::StreamExt;

        let state = Arc::new(crate::AppState::for_test(std::path::PathBuf::from(
            "/tmp/nonexistent-sse-test",
        )));

        // Advance the server two generations while no client is connected.
        // A probe subscriber makes publication observable, so the test never
        // has to guess how many scheduler turns the coalescer needs.
        let mut probe = state.changes.subscribe();
        for expected in 1..=2 {
            state.changes.notify();
            assert_eq!(probe.recv().await.unwrap(), expected);
        }
        assert_eq!(state.changes.current_generation(), 2);

        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", HeaderValue::from_static("1"));
        let response = api_events(State(state.clone()), headers).await;
        assert_eq!(response.status(), StatusCode::OK);

        let mut body = response.into_body().into_data_stream();
        let frame = tokio::time::timeout(std::time::Duration::from_secs(1), body.next())
            .await
            .expect("resync frame must be emitted without waiting for a new change")
            .expect("stream must yield")
            .expect("frame must not error");
        let frame = String::from_utf8_lossy(&frame).into_owned();
        assert!(
            frame.contains("id: 2"),
            "expected resync at generation 2: {frame}"
        );
        assert!(
            frame.contains("resync"),
            "expected a resync reason: {frame}"
        );
    }

    /// An up-to-date client must not be told to refetch on connect.
    #[tokio::test(start_paused = true)]
    async fn reconnect_at_the_current_generation_emits_nothing() {
        use futures::StreamExt;

        let state = Arc::new(crate::AppState::for_test(std::path::PathBuf::from(
            "/tmp/nonexistent-sse-test",
        )));
        let mut probe = state.changes.subscribe();
        state.changes.notify();
        assert_eq!(probe.recv().await.unwrap(), 1);

        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", HeaderValue::from_static("1"));
        let response = api_events(State(state.clone()), headers).await;
        let mut body = response.into_body().into_data_stream();

        let idle = tokio::time::timeout(std::time::Duration::from_millis(50), body.next()).await;
        assert!(
            idle.is_err(),
            "a client already at the current generation must receive no resync"
        );
    }

    /// A live-reload stream must end when shutdown begins.
    ///
    /// `axum::serve(..).with_graceful_shutdown(..)` waits for in-flight
    /// connections after the signal arrives, and an SSE stream never ends on its
    /// own. Without this the server survives SIGTERM for as long as any browser
    /// tab is open, and `kanban web stop` falls through to SIGKILL.
    #[tokio::test]
    async fn shutdown_ends_open_live_reload_streams() {
        use futures::StreamExt;

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let mut state =
            crate::AppState::for_test(std::path::PathBuf::from("/tmp/nonexistent-sse-test"));
        state.shutdown = shutdown_rx;
        let state = Arc::new(state);

        let response = api_events(State(state.clone()), HeaderMap::new()).await;
        let mut body = response.into_body().into_data_stream();

        // Nothing changed, so the stream is parked on the broadcast receiver.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), body.next())
                .await
                .is_err()
        );

        shutdown_tx.send(true).expect("signal shutdown");
        let ended = tokio::time::timeout(Duration::from_secs(1), body.next())
            .await
            .expect("the stream must end promptly on shutdown");
        assert!(ended.is_none(), "the stream must terminate, not emit");
        assert_eq!(
            state.sse_subscribers.load(Ordering::SeqCst),
            0,
            "ending the stream must release the subscriber slot"
        );
    }

    /// The subscriber guard must outlive the handler: it lives in the stream so
    /// `SSE_SUBSCRIBER_CAP` bounds concurrently open streams, not concurrently
    /// executing handlers.
    #[tokio::test]
    async fn subscriber_count_stays_held_while_the_stream_is_open() {
        let state = Arc::new(crate::AppState::for_test(std::path::PathBuf::from(
            "/tmp/nonexistent-sse-test",
        )));
        let response = api_events(State(state.clone()), HeaderMap::new()).await;
        assert_eq!(
            state.sse_subscribers.load(Ordering::SeqCst),
            1,
            "an open stream must count as a subscriber"
        );
        drop(response);
        assert_eq!(
            state.sse_subscribers.load(Ordering::SeqCst),
            0,
            "closing the stream must release the slot"
        );
    }

    #[tokio::test]
    async fn sse_subscriber_cap_rejects_over_limit() {
        let mut state =
            crate::AppState::for_test(std::path::PathBuf::from("/tmp/nonexistent-csrf-test"));
        state.sse_subscribers = Arc::new(AtomicUsize::new(SSE_SUBSCRIBER_CAP));
        let state = Arc::new(state);
        let response = api_events(State(state.clone()), HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            state.sse_subscribers.load(Ordering::SeqCst),
            SSE_SUBSCRIBER_CAP
        );
    }
}
