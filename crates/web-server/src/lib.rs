use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{Days, Local, NaiveDate};
use futures::Stream;
use include_dir::{Dir, include_dir};
use kanban_core::*;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

static WEB_ASSETS: Dir<'_> = include_dir!("$KANBAN_WEB_ASSET_DIR");

const BOARD_STATUSES: [&str; 5] = ["todo", "in-progress", "ready-for-qa", "done", "blocked"];

#[derive(Debug, Clone)]
pub struct WebServeOptions {
    pub repo_root: PathBuf,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
struct AppState {
    repo_root: PathBuf,
    host: String,
    port: u16,
    events: broadcast::Sender<()>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebTaskSummary {
    todo: usize,
    in_progress: usize,
    ready_for_qa: usize,
    done: usize,
    blocked: usize,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
struct WebTask {
    id: String,
    title: String,
    status: String,
    tags: Vec<String>,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebStory {
    id: String,
    title: String,
    status: String,
    phase: Option<String>,
    epic: Option<String>,
    sprint: Option<String>,
    priority: Option<i64>,
    story_points: Option<i64>,
    assignee: Option<String>,
    assignees: Vec<String>,
    work_started: Option<String>,
    work_done: Option<String>,
    activated: Option<String>,
    created: Option<String>,
    updated: Option<String>,
    relative_path: String,
    tasks: Vec<WebTask>,
    task_summary: WebTaskSummary,
    frontmatter: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebStoryDetail {
    #[serde(flatten)]
    story: WebStory,
    body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSprint {
    name: String,
    id: String,
    headline: String,
    goal: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    status: Option<String>,
    wip_limit: Option<i64>,
    stories_by_status: BTreeMap<String, Vec<WebStory>>,
}

#[derive(Debug, Clone, Serialize)]
struct WebEpic {
    id: String,
    title: String,
    phase: String,
    priority: Option<i64>,
    stories: Vec<WebStory>,
}

#[derive(Debug, Serialize)]
struct WebEpicDetail {
    #[serde(flatten)]
    epic: WebEpic,
    body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseSummary {
    phase: String,
    done_points: i64,
    total_points: i64,
    done_stories: usize,
    total_stories: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectProgress {
    done_points: i64,
    total_points: i64,
    done_stories: usize,
    total_stories: usize,
    phases: Vec<PhaseSummary>,
}

#[derive(Debug, Serialize)]
struct RepositorySnapshot {
    stories: Vec<WebStory>,
    epics: Vec<WebEpic>,
    sprints: Vec<WebSprint>,
    progress: ProjectProgress,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebTeamMember {
    name: String,
    email: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    port: u16,
    host: String,
    style: String,
    version: String,
    branch: String,
    story_points: StoryPointsResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoryPointsResponse {
    allowed_values: Vec<String>,
    aliases: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardMetrics {
    burndown: Vec<BurndownPoint>,
    burnup: Vec<BurnupPoint>,
    lead_time: Vec<LeadTimePoint>,
    velocity: Vec<VelocityPoint>,
    forecast: Forecast,
    progress: ProjectProgress,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct BurndownPoint {
    date: String,
    remaining: i64,
    ideal: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct BurnupPoint {
    date: String,
    completed: i64,
    scope: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LeadTimePoint {
    story_id: String,
    date: String,
    days: i64,
    rolling_avg: f64,
}

#[derive(Debug, Serialize)]
struct VelocityPoint {
    sprint: String,
    points: i64,
    forecast: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Forecast {
    generated_at: String,
    remaining_points: i64,
    sprint_duration_weeks: i64,
    projection_start_date: String,
    throughput: ForecastThroughput,
    completion: ForecastCompletion,
    confidence: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForecastThroughput {
    samples: Vec<i64>,
    average: f64,
    median: f64,
    observed_day_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForecastCompletion {
    p50_days: Option<i64>,
    p80_days: Option<i64>,
    p90_days: Option<i64>,
    p50_date: Option<String>,
    p80_date: Option<String>,
    p90_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveInput {
    status: String,
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlanInput {
    sprint: String,
}

#[derive(Debug, Deserialize)]
struct UpdateTaskInput {
    status: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateBodyInput {
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStoryFieldsInput {
    assignee: Option<String>,
    sprint: Option<String>,
    status: Option<String>,
    story_points: Option<Value>,
    priority: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateEpicFieldsInput {
    priority: i64,
}

#[derive(Debug, Deserialize)]
struct CreateSprintInputWeb {
    headline: String,
    number: Option<u32>,
    start: Option<String>,
    end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSprintInput {
    headline: String,
    goal: String,
    start: String,
    end: String,
    status: String,
    wip_limit: Option<i64>,
}

pub fn serve_blocking(options: WebServeOptions) -> Result<()> {
    tokio::runtime::Runtime::new()
        .context("create kanban web runtime")?
        .block_on(serve(options))
}

pub async fn serve(options: WebServeOptions) -> Result<()> {
    let config = load_kanban_config(&options.repo_root)?;
    let repo_root = config.repo_root;
    let (events, _) = broadcast::channel(128);
    let _watcher = start_watcher(&repo_root, events.clone())?;
    let state = Arc::new(AppState {
        repo_root,
        host: options.host,
        port: options.port,
        events,
    });
    let app = Router::new()
        .route("/api/repository", get(api_repository))
        .route("/api/metrics", get(api_metrics))
        .route("/api/config", get(api_config))
        .route("/api/team", get(api_team))
        .route("/api/team/avatars/{*path}", get(api_team_avatar))
        .route("/api/epics/{id}", get(api_epic))
        .route("/api/epics/{id}/fields", patch(api_update_epic_fields))
        .route(
            "/api/stories/{id}",
            get(api_story).put(api_update_story_body),
        )
        .route("/api/stories/{id}/fields", patch(api_update_story_fields))
        .route("/api/stories/{id}/tasks/{task_id}", patch(api_update_task))
        .route("/api/stories/{id}/move", post(api_move_story))
        .route("/api/stories/{id}/plan", post(api_plan_story))
        .route("/api/sprints", post(api_create_sprint))
        .route("/api/sprints/{name}", post(api_update_sprint))
        .route("/api/events", get(api_events))
        .fallback(static_asset)
        .with_state(state.clone());

    let addr: SocketAddr = format!("{}:{}", state.host, state.port)
        .parse()
        .with_context(|| format!("parse web bind address {}:{}", state.host, state.port))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind kanban web server to {addr}"))?;
    println!(
        "kanban-web listening on http://{}:{} (repo: {})",
        state.host,
        state.port,
        state.repo_root.display()
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("run kanban web server")
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

fn start_watcher(repo_root: &Path, events: broadcast::Sender<()>) -> Result<RecommendedWatcher> {
    let config = load_kanban_config(repo_root)?;
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            let _ = events.send(());
        }
    })?;
    if config.backlog_path().exists() {
        watcher.watch(&config.backlog_path(), RecursiveMode::Recursive)?;
    }
    if config.sprints_path().exists() {
        watcher.watch(&config.sprints_path(), RecursiveMode::Recursive)?;
    }
    Ok(watcher)
}

async fn api_repository(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RepositorySnapshot>, ApiResponse> {
    Ok(Json(load_repository_snapshot(&state.repo_root)?))
}

async fn api_metrics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DashboardMetrics>, ApiResponse> {
    let repo = load_repository_snapshot(&state.repo_root)?;
    Ok(Json(compute_metrics(&repo)))
}

async fn api_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ConfigResponse>, ApiResponse> {
    let config = load_kanban_config(&state.repo_root)?;
    Ok(Json(ConfigResponse {
        port: state.port,
        host: state.host.clone(),
        style: config.web.style,
        version: env!("CARGO_PKG_VERSION").to_string(),
        branch: git_branch(&config.repo_root),
        story_points: StoryPointsResponse {
            allowed_values: config.story_points.allowed_values,
            aliases: config.story_points.aliases,
        },
    }))
}

async fn api_team(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<WebTeamMember>>, ApiResponse> {
    Ok(Json(load_team(&state.repo_root)?))
}

async fn api_team_avatar(
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

async fn api_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<WebStoryDetail>, ApiResponse> {
    let (story, body) = load_story_detail(&state.repo_root, &id)?
        .ok_or_else(|| ApiResponse::not_found("not found"))?;
    Ok(Json(WebStoryDetail { story, body }))
}

async fn api_epic(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<WebEpicDetail>, ApiResponse> {
    let (epic, body) = load_epic_detail(&state.repo_root, &id)?
        .ok_or_else(|| ApiResponse::not_found("not found"))?;
    Ok(Json(WebEpicDetail { epic, body }))
}

async fn api_move_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<MoveInput>,
) -> Result<Json<Value>, ApiResponse> {
    let result = move_story_to_status_with_assignee(
        &state.repo_root,
        &id,
        &input.status,
        input.assignee.as_deref(),
    )?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": MoveStoryDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_plan_story(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<PlanInput>,
) -> Result<Json<Value>, ApiResponse> {
    let result = plan_story_into_sprint(&state.repo_root, &id, &input.sprint)?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": PlanStoryDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_update_task(
    State(state): State<Arc<AppState>>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
    Json(input): Json<UpdateTaskInput>,
) -> Result<Json<Value>, ApiResponse> {
    let tags = input.tags.as_deref().map(parse_tags).unwrap_or_default();
    let tags_ref = if input.tags.is_some() {
        Some(tags.as_slice())
    } else {
        None
    };
    let result = update_task_in_story(
        &state.repo_root,
        &id,
        &task_id,
        input.status.as_deref(),
        input.title.as_deref(),
        tags_ref,
        input.description.as_deref(),
    )?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": TaskMutationDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_update_story_body(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateBodyInput>,
) -> Result<Json<Value>, ApiResponse> {
    let (_, source) = find_story_with_source(&state.repo_root, &id)?
        .ok_or_else(|| ApiResponse::not_found("not found"))?;
    let updated = replace_markdown_body(&source.markdown, &input.body);
    fs::write(&source.file_path, updated)
        .with_context(|| format!("write story file {}", source.file_path.display()))?;
    let _ = state.events.send(());
    Ok(Json(json!({ "ok": true })))
}

async fn api_update_story_fields(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateStoryFieldsInput>,
) -> Result<Json<Value>, ApiResponse> {
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
    let result = update_story_frontmatter(&state.repo_root, &id, &updates)?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": StoryUpdateDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_update_epic_fields(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<UpdateEpicFieldsInput>,
) -> Result<Json<Value>, ApiResponse> {
    let updates = [("priority".to_string(), input.priority.to_string())];
    let result = update_epic_frontmatter(&state.repo_root, &id, &updates)?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": EpicUpdateDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_create_sprint(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateSprintInputWeb>,
) -> Result<Json<Value>, ApiResponse> {
    let number = input
        .number
        .unwrap_or(suggested_next_sprint_number(&state.repo_root)?);
    let (default_start, default_end) = suggested_next_sprint_dates(&state.repo_root)?
        .unwrap_or_else(|| {
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
    let result = create_sprint(&state.repo_root, &create_input)?;
    let _ = state.events.send(());
    Ok(Json(
        json!({ "ok": true, "data": SprintCreateDto::from_result(&result, &state.repo_root) }),
    ))
}

async fn api_update_sprint(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<UpdateSprintInput>,
) -> Result<Json<Value>, ApiResponse> {
    let result = update_sprint_file(&state.repo_root, &name, input)?;
    let _ = state.events.send(());
    Ok(Json(json!({ "ok": true, "data": result })))
}

async fn api_events(
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

async fn static_asset(uri: Uri) -> Response {
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

#[derive(Debug)]
struct ApiResponse {
    status: StatusCode,
    message: String,
}

impl ApiResponse {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
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

fn load_repository_snapshot(repo_root: &Path) -> Result<RepositorySnapshot> {
    let repository = read_repository(repo_root)?;
    let mut stories = repository
        .stories
        .iter()
        .map(|story| web_story_from_core(&repository.repo_root, story))
        .collect::<Vec<_>>();
    stories.sort_by(|a, b| a.id.cmp(&b.id));
    let epics = load_epics(&repository.repo_root, &stories)?;
    let sprints = load_sprints(&repository.repo_root, &stories)?;
    let progress = compute_progress(&stories);
    Ok(RepositorySnapshot {
        stories,
        epics,
        sprints,
        progress,
    })
}

fn web_story_from_core(repo_root: &Path, story: &kanban_core::Story) -> WebStory {
    let id = story.frontmatter.get("id").cloned().unwrap_or_default();
    let tasks = story
        .task_file
        .as_ref()
        .map(|task_file| {
            task_file
                .tasks
                .iter()
                .map(web_task_from_core)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let assignee = empty_to_none(story.frontmatter.get("assignee"));
    let assignees = assignee.as_deref().map(parse_assignees).unwrap_or_default();
    WebStory {
        title: title_from_body(&story.body, "User Story"),
        status: story.frontmatter.get("status").cloned().unwrap_or_default(),
        phase: phase_from_id(&id, "US"),
        epic: empty_to_none(story.frontmatter.get("epic")),
        sprint: empty_to_none(story.frontmatter.get("sprint")),
        priority: story
            .frontmatter
            .get("priority")
            .and_then(|value| parse_non_negative_i64(value)),
        story_points: story
            .frontmatter
            .get("story_points")
            .and_then(|value| parse_i64(value)),
        assignee,
        assignees,
        work_started: empty_to_none(story.frontmatter.get("work_started")),
        work_done: empty_to_none(story.frontmatter.get("work_done")),
        activated: empty_to_none(story.frontmatter.get("activated")),
        created: empty_to_none(story.frontmatter.get("created")),
        updated: empty_to_none(story.frontmatter.get("updated")),
        relative_path: rel_to_root(repo_root, &story.relative_path),
        task_summary: summarize_web_tasks(&tasks),
        tasks,
        frontmatter: story.frontmatter.clone(),
        id,
    }
}

fn web_task_from_core(task: &kanban_core::Task) -> WebTask {
    WebTask {
        id: task.id.clone(),
        title: task.title.clone(),
        status: task.normalized_status.clone(),
        tags: task.tags.clone(),
        description: task.description.clone(),
    }
}

fn summarize_web_tasks(tasks: &[WebTask]) -> WebTaskSummary {
    let mut summary = WebTaskSummary {
        todo: 0,
        in_progress: 0,
        ready_for_qa: 0,
        done: 0,
        blocked: 0,
        total: tasks.len(),
    };
    for task in tasks {
        match task.status.as_str() {
            "in-progress" => summary.in_progress += 1,
            "ready-for-qa" => summary.ready_for_qa += 1,
            "done" => summary.done += 1,
            "blocked" => summary.blocked += 1,
            _ => summary.todo += 1,
        }
    }
    summary
}

fn load_story_detail(repo_root: &Path, id: &str) -> Result<Option<(WebStory, String)>> {
    Ok(find_story_with_source(repo_root, id)?.map(|(_, source)| {
        let story = web_story_from_core(repo_root, &source);
        (story, source.body)
    }))
}

fn load_epic_detail(repo_root: &Path, id: &str) -> Result<Option<(WebEpic, String)>> {
    let repository = load_repository_snapshot(repo_root)?;
    let Some(mut epic) = repository
        .epics
        .into_iter()
        .find(|epic| epic.id.eq_ignore_ascii_case(id))
    else {
        return Ok(None);
    };
    let source = find_epic_with_source(repo_root, id)?;
    let body = source.map(|(_, source)| source.body).unwrap_or_default();
    epic.stories.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Some((epic, body)))
}

fn load_epics(repo_root: &Path, stories: &[WebStory]) -> Result<Vec<WebEpic>> {
    let mut epics = BTreeMap::<String, WebEpic>::new();
    for path in collect_epic_files(repo_root)? {
        let source = read_epic_file(&path, repo_root)?;
        let id = source.frontmatter.get("id").cloned().unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        epics.insert(
            id.clone(),
            WebEpic {
                title: title_from_body(&source.body, "Epic"),
                phase: phase_from_id(&id, "EP").unwrap_or_else(|| "F?".to_string()),
                priority: source
                    .frontmatter
                    .get("priority")
                    .and_then(|value| parse_non_negative_i64(value)),
                stories: Vec::new(),
                id,
            },
        );
    }
    for story in stories {
        if let Some(epic_id) = &story.epic {
            let entry = epics.entry(epic_id.clone()).or_insert_with(|| WebEpic {
                id: epic_id.clone(),
                title: epic_id.clone(),
                phase: phase_from_id(epic_id, "EP")
                    .unwrap_or_else(|| story.phase.clone().unwrap_or_else(|| "F?".to_string())),
                priority: None,
                stories: Vec::new(),
            });
            entry.stories.push(story.clone());
        }
    }
    Ok(epics.into_values().collect())
}

fn load_sprints(repo_root: &Path, stories: &[WebStory]) -> Result<Vec<WebSprint>> {
    let config = load_kanban_config(repo_root)?;
    let mut sprints = Vec::new();
    let Ok(entries) = fs::read_dir(config.sprints_path()) else {
        return Ok(sprints);
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !stem.starts_with('S') || !stem.contains('.') {
            continue;
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("read sprint file {}", path.display()))?;
        let parsed = parse_frontmatter(&content);
        let mut by_status = BOARD_STATUSES
            .iter()
            .map(|status| ((*status).to_string(), Vec::<WebStory>::new()))
            .collect::<BTreeMap<_, _>>();
        for story in stories
            .iter()
            .filter(|story| story.sprint.as_deref() == Some(stem))
        {
            if let Some(bucket) = by_status.get_mut(&story.status) {
                bucket.push(story.clone());
            }
        }
        for bucket in by_status.values_mut() {
            bucket.sort_by(|a, b| {
                priority_sort_key(a)
                    .cmp(&priority_sort_key(b))
                    .then_with(|| a.id.cmp(&b.id))
            });
        }
        sprints.push(WebSprint {
            name: stem.to_string(),
            id: parsed
                .frontmatter
                .get("sprint")
                .cloned()
                .unwrap_or_else(|| stem.split('.').next().unwrap_or(stem).to_string()),
            headline: parsed
                .frontmatter
                .get("headline")
                .cloned()
                .unwrap_or_default(),
            goal: extract_section(&parsed.body, "Sprint Goal"),
            start_date: empty_to_none(parsed.frontmatter.get("start_date")),
            end_date: empty_to_none(parsed.frontmatter.get("end_date")),
            status: empty_to_none(parsed.frontmatter.get("status")),
            wip_limit: parsed
                .frontmatter
                .get("wip_limit")
                .and_then(|value| parse_non_negative_i64(value)),
            stories_by_status: by_status,
        });
    }
    sprints.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sprints)
}

fn compute_progress(stories: &[WebStory]) -> ProjectProgress {
    let mut phases = BTreeMap::<String, PhaseSummary>::new();
    let mut done_points = 0;
    let mut total_points = 0;
    let mut done_stories = 0;
    for story in stories {
        let points = story.story_points.unwrap_or(0);
        let phase = story.phase.clone().unwrap_or_else(|| "F?".to_string());
        let entry = phases.entry(phase.clone()).or_insert(PhaseSummary {
            phase,
            done_points: 0,
            total_points: 0,
            done_stories: 0,
            total_stories: 0,
        });
        entry.total_points += points;
        entry.total_stories += 1;
        total_points += points;
        if story.status == "done" {
            entry.done_points += points;
            entry.done_stories += 1;
            done_points += points;
            done_stories += 1;
        }
    }
    ProjectProgress {
        done_points,
        total_points,
        done_stories,
        total_stories: stories.len(),
        phases: phases.into_values().collect(),
    }
}

fn compute_metrics(repo: &RepositorySnapshot) -> DashboardMetrics {
    let progress = compute_progress(&repo.stories);
    DashboardMetrics {
        burndown: build_burndown(&repo.sprints),
        burnup: build_burnup(&repo.stories, &repo.sprints),
        lead_time: build_lead_time(&repo.stories),
        velocity: build_velocity(&repo.sprints),
        forecast: build_forecast(&repo.stories, &repo.sprints),
        progress,
    }
}

fn build_burnup(stories: &[WebStory], sprints: &[WebSprint]) -> Vec<BurnupPoint> {
    let today = Local::now().date_naive();
    let mut completed_by_date = BTreeMap::<NaiveDate, i64>::new();
    for story in stories.iter().filter(|story| story.status == "done") {
        let Some(date) = story_completion_date(story) else {
            continue;
        };
        *completed_by_date.entry(date).or_default() += story.story_points.unwrap_or(0);
    }

    let mut scope_changes = BTreeMap::<NaiveDate, i64>::new();
    let mut sprint_boundaries = BTreeSet::<NaiveDate>::new();
    for sprint in sprints {
        let Some(start_date) = sprint.start_date.as_deref().and_then(parse_date_prefix) else {
            continue;
        };
        if start_date > today {
            continue;
        }
        sprint_boundaries.insert(start_date);
        if let Some(end_date) = sprint.end_date.as_deref().and_then(parse_date_prefix)
            && end_date <= today
        {
            sprint_boundaries.insert(end_date);
        }
        *scope_changes.entry(start_date).or_default() += sprint_total_points(sprint);
    }

    let start_date = stories
        .iter()
        .filter_map(story_work_started_date)
        .min()
        .or_else(|| completed_by_date.keys().next().copied())
        .or_else(|| scope_changes.keys().next().copied());
    let Some(start_date) = start_date else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    let mut cumulative = completed_by_date
        .range(..start_date)
        .map(|(_, points)| *points)
        .sum::<i64>();
    let mut scope = scope_changes
        .range(..=start_date)
        .map(|(_, points)| *points)
        .sum::<i64>();
    rows.push(BurnupPoint {
        date: start_date.to_string(),
        completed: cumulative,
        scope,
    });

    let dates = completed_by_date
        .keys()
        .chain(scope_changes.keys())
        .chain(sprint_boundaries.iter())
        .copied()
        .filter(|date| *date >= start_date && *date <= today)
        .collect::<BTreeSet<_>>();
    for date in dates {
        if date > start_date {
            scope += scope_changes.get(&date).copied().unwrap_or(0);
            cumulative += completed_by_date.get(&date).copied().unwrap_or(0);
            rows.push(BurnupPoint {
                date: date.to_string(),
                completed: cumulative,
                scope,
            });
            continue;
        }

        cumulative += completed_by_date.get(&date).copied().unwrap_or(0);
        if let Some(last) = rows.last_mut() {
            last.completed = cumulative;
            last.scope = scope;
        }
    }

    if rows
        .last()
        .is_some_and(|last| last.date != today.to_string())
    {
        rows.push(BurnupPoint {
            date: today.to_string(),
            completed: cumulative,
            scope,
        });
    }

    rows
}

fn build_burndown(sprints: &[WebSprint]) -> Vec<BurndownPoint> {
    let Some(sprint) = select_burndown_sprint(sprints) else {
        return Vec::new();
    };
    let Some(start_date) = sprint.start_date.as_deref().and_then(parse_date_prefix) else {
        return Vec::new();
    };
    let Some(end_date) = sprint.end_date.as_deref().and_then(parse_date_prefix) else {
        return Vec::new();
    };
    if end_date < start_date {
        return Vec::new();
    }

    let planned_points = sprint_total_points(sprint);
    if planned_points <= 0 {
        return Vec::new();
    }

    let today = Local::now().date_naive();
    let last_date = match sprint.status.as_deref() {
        Some("closed") => end_date,
        _ => std::cmp::min(end_date, std::cmp::max(start_date, today)),
    };

    let mut completed_by_date = BTreeMap::<NaiveDate, i64>::new();
    for story in sprint.stories_by_status.values().flatten() {
        if story.status != "done" {
            continue;
        }
        let Some(date) = story_completion_date(story) else {
            continue;
        };
        *completed_by_date
            .entry(std::cmp::min(date, end_date))
            .or_default() += story.story_points.unwrap_or(0);
    }

    let total_days = (end_date - start_date).num_days();
    let visible_days = (last_date - start_date).num_days();
    let mut rows = Vec::new();
    let mut completed = 0;
    for offset in 0..=visible_days {
        let date = start_date + Days::new(offset as u64);
        completed += completed_by_date.get(&date).copied().unwrap_or(0);
        let remaining = (planned_points - completed).max(0);
        let ideal = if total_days <= 0 {
            0
        } else {
            (((planned_points as f64) * (1.0 - (offset as f64 / total_days as f64))).round() as i64)
                .max(0)
        };
        rows.push(BurndownPoint {
            date: date.to_string(),
            remaining,
            ideal,
        });
    }

    rows
}

fn select_burndown_sprint(sprints: &[WebSprint]) -> Option<&WebSprint> {
    sprints
        .iter()
        .find(|sprint| sprint.status.as_deref() == Some("active"))
        .or_else(|| {
            sprints.iter().rev().find(|sprint| {
                sprint.status.as_deref() != Some("closed") && sprint_total_points(sprint) > 0
            })
        })
        .or_else(|| {
            sprints
                .iter()
                .rev()
                .find(|sprint| sprint_total_points(sprint) > 0)
        })
        .or_else(|| sprints.last())
}

fn sprint_total_points(sprint: &WebSprint) -> i64 {
    sprint
        .stories_by_status
        .values()
        .flatten()
        .map(|story| story.story_points.unwrap_or(0))
        .sum()
}

fn story_work_started_date(story: &WebStory) -> Option<NaiveDate> {
    story.work_started.as_deref().and_then(parse_date_prefix)
}

fn story_completion_date(story: &WebStory) -> Option<NaiveDate> {
    story
        .work_done
        .as_deref()
        .and_then(parse_date_prefix)
        .or_else(|| story.updated.as_deref().and_then(parse_date_prefix))
        .or_else(|| story.created.as_deref().and_then(parse_date_prefix))
}

fn build_lead_time(stories: &[WebStory]) -> Vec<LeadTimePoint> {
    let mut done = stories
        .iter()
        .filter(|story| {
            story.status == "done" && story.work_started.is_some() && story.work_done.is_some()
        })
        .collect::<Vec<_>>();
    done.sort_by(|a, b| a.work_done.cmp(&b.work_done));
    let mut window = Vec::<i64>::new();
    let mut points = Vec::new();
    for story in done {
        let days = days_between(
            story.work_started.as_deref().unwrap_or_default(),
            story.work_done.as_deref().unwrap_or_default(),
        )
        .unwrap_or(0);
        window.push(days);
        if window.len() > 7 {
            window.remove(0);
        }
        let rolling_avg = window.iter().sum::<i64>() as f64 / window.len() as f64;
        points.push(LeadTimePoint {
            story_id: story.id.clone(),
            date: story.work_done.clone().unwrap_or_default(),
            days,
            rolling_avg,
        });
    }
    points
}

fn build_velocity(sprints: &[WebSprint]) -> Vec<VelocityPoint> {
    sprints
        .iter()
        .map(|sprint| VelocityPoint {
            sprint: sprint.name.clone(),
            points: sprint
                .stories_by_status
                .get("done")
                .map(|stories| {
                    stories
                        .iter()
                        .map(|story| story.story_points.unwrap_or(0))
                        .sum()
                })
                .unwrap_or(0),
            forecast: false,
        })
        .collect()
}

fn build_forecast(stories: &[WebStory], sprints: &[WebSprint]) -> Forecast {
    let story_overviews = stories
        .iter()
        .map(story_overview_from_web)
        .collect::<Vec<_>>();
    let sprint_overviews = sprints
        .iter()
        .map(sprint_overview_from_web)
        .collect::<Vec<_>>();
    let current_sprint_name = sprints
        .iter()
        .find(|sprint| sprint.status.as_deref() == Some("active"))
        .map(|sprint| sprint.name.as_str());
    let canonical =
        ReportForecastDto::build(&story_overviews, &sprint_overviews, current_sprint_name);
    Forecast::from(canonical)
}

fn story_overview_from_web(story: &WebStory) -> StoryOverview {
    StoryOverview {
        id: story.id.clone(),
        title: story.title.clone(),
        status: story.status.clone(),
        epic_id: story.epic.clone(),
        epic_title: None,
        assignee: story.assignee.clone().unwrap_or_default(),
        story_points: story
            .story_points
            .map(|points| points.to_string())
            .unwrap_or_default(),
        sprint: story.sprint.clone(),
        relative_path: PathBuf::from(&story.relative_path),
        task_summary: Some(TaskSummary {
            todo: story.task_summary.todo,
            in_progress: story.task_summary.in_progress,
            blocked: story.task_summary.blocked,
            done: story.task_summary.done,
        }),
        task_count: story.task_summary.total,
        work_started: story.work_started.clone(),
        work_done: story.work_done.clone(),
        planned_start: None,
        planned_end: None,
    }
}

fn sprint_overview_from_web(sprint: &WebSprint) -> SprintOverview {
    SprintOverview {
        sprint_name: sprint.name.clone(),
        headline: sprint.headline.clone(),
        sprint_goal: sprint.goal.clone(),
        start_date: sprint.start_date.clone().unwrap_or_default(),
        end_date: sprint.end_date.clone().unwrap_or_default(),
        readme_path: PathBuf::from(format!("delivery/sprints/{}.md", sprint.name)),
        readme_status: sprint.status.clone(),
        stories_by_status: sprint
            .stories_by_status
            .iter()
            .map(|(status, stories)| {
                (
                    status.clone(),
                    stories
                        .iter()
                        .map(story_overview_from_web)
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
        blocked_work: Vec::new(),
        warnings: Vec::new(),
    }
}

impl From<ReportForecastDto> for Forecast {
    fn from(value: ReportForecastDto) -> Self {
        Self {
            generated_at: value.generated_at,
            remaining_points: value.remaining_points,
            sprint_duration_weeks: value.sprint_duration_weeks as i64,
            projection_start_date: value.projection_start_date,
            throughput: ForecastThroughput {
                samples: value.throughput.samples,
                average: value.throughput.average,
                median: value.throughput.median,
                observed_day_count: value.throughput.observed_day_count,
            },
            completion: ForecastCompletion {
                p50_days: value.completion.p50_days.map(i64::from),
                p80_days: value.completion.p80_days.map(i64::from),
                p90_days: value.completion.p90_days.map(i64::from),
                p50_date: value.completion.p50_date,
                p80_date: value.completion.p80_date,
                p90_date: value.completion.p90_date,
            },
            confidence: value.confidence,
        }
    }
}

fn load_team(repo_root: &Path) -> Result<Vec<WebTeamMember>> {
    let team_file = repo_root.join(".kanban/team.json");
    if let Ok(raw) = fs::read_to_string(&team_file)
        && let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&raw)
    {
        let avatars_prefix = "/api/team/avatars/".to_string();
        return Ok(values
            .into_iter()
            .filter_map(|item| {
                let obj = item.as_object()?;
                let name = obj.get("name")?.as_str()?.trim().to_string();
                let email = obj.get("email")?.as_str()?.trim().to_string();
                if name.is_empty() || email.is_empty() {
                    return None;
                }
                let label = format!("{name} <{email}>");
                let avatar_url = obj
                    .get("avatarUrl")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        obj.get("avatarPath")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|path| format!("{avatars_prefix}{}", path.trim_start_matches('/')))
                    });
                Some(WebTeamMember {
                    name,
                    email,
                    label,
                    avatar_url,
                })
            })
            .collect());
    }

    let repository = read_repository(repo_root)?;
    let mut seen: BTreeMap<String, (String, String)> = BTreeMap::new();
    for story in repository.stories {
        if let Some(assignee) = story.frontmatter.get("assignee") {
            for person in parse_assignees(assignee) {
                if person.contains('<')
                    && person.contains('@')
                    && !person.eq_ignore_ascii_case("Name <email@example.com>")
                    && let Some((name, email)) = person.split_once('<')
                {
                    let name = name.trim().to_string();
                    let email = email.trim_end_matches('>').trim().to_string();
                    if !name.is_empty() && !email.is_empty() {
                        seen.entry(email.clone()).or_insert((name, email));
                    }
                }
            }
        }
    }
    Ok(seen
        .into_values()
        .map(|(name, email)| WebTeamMember {
            label: format!("{name} <{email}>"),
            avatar_url: None,
            name,
            email,
        })
        .collect())
}

fn update_sprint_file(repo_root: &Path, name: &str, input: UpdateSprintInput) -> Result<Value> {
    let config = load_kanban_config(repo_root)?;
    let old_path = config.sprints_path().join(format!("{name}.md"));
    let content = fs::read_to_string(&old_path)
        .with_context(|| format!("read sprint file {}", old_path.display()))?;
    let parsed = parse_frontmatter(&content);
    let sprint_id = parsed
        .frontmatter
        .get("sprint")
        .cloned()
        .unwrap_or_else(|| name.split('.').next().unwrap_or(name).to_string());
    let headline = slugify(&input.headline);
    if headline.is_empty() {
        bail!("Sprint headline must contain at least one ASCII letter or number.");
    }
    let new_name = format!("{sprint_id}.{headline}");
    let new_path = config.sprints_path().join(format!("{new_name}.md"));
    if new_name != name && new_path.exists() {
        bail!("Sprint file already exists: {new_name}.md");
    }
    let mut updates = BTreeMap::new();
    updates.insert("headline".to_string(), headline.clone());
    updates.insert("start_date".to_string(), input.start);
    updates.insert("end_date".to_string(), input.end);
    updates.insert("status".to_string(), input.status);
    updates.insert(
        "wip_limit".to_string(),
        input
            .wip_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string()),
    );
    let mut updated = replace_frontmatter_fields(&content, &updates)?;
    updated = replace_section_content(&updated, "Sprint Goal", &input.goal);
    updated = replace_sprint_title(&updated, &sprint_id, &headline);
    fs::write(&old_path, updated)
        .with_context(|| format!("write sprint file {}", old_path.display()))?;
    if new_name != name {
        update_story_sprint_references(repo_root, name, &new_name)?;
        fs::rename(&old_path, &new_path)
            .with_context(|| format!("rename sprint file to {}", new_path.display()))?;
    }
    Ok(
        json!({ "sprintPath": rel_to_root(repo_root, &new_path), "name": new_name, "headline": headline }),
    )
}

fn update_story_sprint_references(repo_root: &Path, old_name: &str, new_name: &str) -> Result<()> {
    let repository = read_repository(repo_root)?;
    for story in repository.stories {
        if story.frontmatter.get("sprint").map(String::as_str) == Some(old_name) {
            let updates = [("sprint".to_string(), new_name.to_string())];
            update_story_frontmatter(
                repo_root,
                story
                    .frontmatter
                    .get("id")
                    .map(String::as_str)
                    .unwrap_or_default(),
                &updates,
            )?;
        }
    }
    Ok(())
}

fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_assignees(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && *value != "~"
                && !value.eq_ignore_ascii_case("TBD")
                && !value.eq_ignore_ascii_case("Name <email@example.com>")
        })
        .map(str::to_string)
        .collect()
}

fn title_from_body(body: &str, prefix: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|title| {
            title
                .trim()
                .strip_prefix(&format!("{prefix}: "))
                .unwrap_or(title.trim())
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

fn phase_from_id(id: &str, prefix: &str) -> Option<String> {
    let marker = format!("{prefix}-F");
    let start = id.to_ascii_uppercase().find(&marker)? + prefix.len() + 1;
    let rest = &id[start..];
    let end = rest.find('-').unwrap_or(rest.len());
    let phase = &rest[..end];
    (!phase.is_empty()).then(|| phase.to_ascii_uppercase())
}

fn empty_to_none(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "~" && *value != "null")
        .map(str::to_string)
}

fn parse_i64(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

fn parse_non_negative_i64(value: &str) -> Option<i64> {
    parse_i64(value).filter(|value| *value >= 0)
}

fn priority_sort_key(story: &WebStory) -> i64 {
    story.priority.unwrap_or(i64::MAX)
}

fn rel_to_root(repo_root: &Path, path: &Path) -> String {
    let path = if path.is_absolute() {
        path.strip_prefix(repo_root).unwrap_or(path)
    } else {
        path
    };
    path.to_string_lossy().replace('\\', "/")
}

fn extract_section(body: &str, heading: &str) -> Option<String> {
    let marker = format!("## {heading}");
    let start = body.find(&marker)? + marker.len();
    let rest = &body[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn git_branch(repo_root: &Path) -> String {
    Command::new("git")
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

fn days_between(start: &str, end: &str) -> Option<i64> {
    let start = parse_date_prefix(start)?;
    let end = parse_date_prefix(end)?;
    Some((end - start).num_days().max(0))
}

fn parse_date_prefix(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()
}

fn parse_date_or(value: Option<&str>, fallback: NaiveDate) -> Result<NaiveDate> {
    match value.filter(|value| !value.trim().is_empty()) {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .with_context(|| format!("parse date {value}")),
        None => Ok(fallback),
    }
}

fn replace_markdown_body(markdown: &str, body: &str) -> String {
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

fn replace_frontmatter_fields(
    markdown: &str,
    updates: &BTreeMap<String, String>,
) -> Result<String> {
    let newline = if markdown.starts_with("---\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !markdown.starts_with("---") {
        bail!("markdown file does not start with frontmatter");
    }
    let close = format!("{newline}---");
    let end = markdown[3..]
        .find(&close)
        .ok_or_else(|| anyhow!("frontmatter is not closed"))?
        + 3;
    let frontmatter = &markdown[..end];
    let rest = &markdown[end..];
    let mut lines = frontmatter.lines().map(str::to_string).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for line in &mut lines {
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim().to_string();
            if let Some(value) = updates.get(&key) {
                *line = format!("{}: {}", key, value);
                seen.insert(key);
            }
        }
    }
    for (key, value) in updates {
        if !seen.contains(key) {
            lines.push(format!("{key}: {value}"));
        }
    }
    Ok(format!("{}{}", lines.join(newline), rest))
}

fn replace_section_content(markdown: &str, heading: &str, value: &str) -> String {
    let marker = format!("## {heading}");
    let Some(index) = markdown.find(&marker) else {
        return markdown.to_string();
    };
    let start = index + marker.len();
    let next = markdown[start..]
        .find("\n## ")
        .map(|offset| start + offset + 1)
        .unwrap_or(markdown.len());
    format!(
        "{}{}\n\n{}\n{}",
        &markdown[..index],
        marker,
        value.trim(),
        &markdown[next..]
    )
}

fn replace_sprint_title(markdown: &str, sprint_id: &str, headline: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            if line.starts_with("# ") {
                format!("# {sprint_id}: {headline}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in value.trim().chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn json_value_to_string(value: Value) -> String {
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

    fn test_story(
        id: &str,
        status: &str,
        story_points: i64,
        created: Option<&str>,
        work_done: Option<&str>,
    ) -> WebStory {
        WebStory {
            id: id.to_string(),
            title: id.to_string(),
            status: status.to_string(),
            phase: Some("F1".to_string()),
            epic: Some("EP-F1-01".to_string()),
            sprint: Some("S001.current".to_string()),
            priority: None,
            story_points: Some(story_points),
            assignee: None,
            assignees: Vec::new(),
            work_started: created.map(str::to_string),
            work_done: work_done.map(str::to_string),
            activated: created.map(str::to_string),
            created: created.map(str::to_string),
            updated: work_done.map(str::to_string),
            relative_path: "story.md".to_string(),
            tasks: Vec::new(),
            task_summary: WebTaskSummary {
                todo: 0,
                in_progress: 0,
                ready_for_qa: 0,
                done: 0,
                blocked: 0,
                total: 0,
            },
            frontmatter: BTreeMap::new(),
        }
    }

    fn test_sprint(status: &str, stories: Vec<WebStory>) -> WebSprint {
        test_sprint_with_start(status, "2026-06-01", stories)
    }

    fn test_sprint_with_start(status: &str, start_date: &str, stories: Vec<WebStory>) -> WebSprint {
        test_sprint_with_dates(status, start_date, "2026-06-05", stories)
    }

    fn test_sprint_with_dates(
        status: &str,
        start_date: &str,
        end_date: &str,
        stories: Vec<WebStory>,
    ) -> WebSprint {
        let mut stories_by_status = BOARD_STATUSES
            .iter()
            .map(|name| ((*name).to_string(), Vec::<WebStory>::new()))
            .collect::<BTreeMap<_, _>>();
        for story in stories {
            stories_by_status
                .get_mut(&story.status)
                .expect("known status bucket")
                .push(story);
        }
        WebSprint {
            name: "S001.current".to_string(),
            id: "S001".to_string(),
            headline: "current".to_string(),
            goal: None,
            start_date: Some(start_date.to_string()),
            end_date: Some(end_date.to_string()),
            status: Some(status.to_string()),
            wip_limit: None,
            stories_by_status,
        }
    }

    #[test]
    fn replace_markdown_body_preserves_frontmatter() {
        let markdown = "---\nid: US-F1-001\n---\n# Old\n";
        let updated = replace_markdown_body(markdown, "# New\n");
        assert!(updated.starts_with("---\nid: US-F1-001\n---\n\n"));
        assert!(updated.ends_with("# New\n"));
    }

    #[test]
    fn slugify_headline_keeps_ascii_tokens() {
        assert_eq!(slugify("Foundation Sprint!"), "foundation-sprint");
    }

    #[test]
    fn build_burnup_starts_at_earliest_work_started_date() {
        let done = test_story(
            "US-F1-001",
            "done",
            5,
            Some("2026-06-01T09:00:00+0200"),
            Some("2026-06-03T12:00:00+0200"),
        );
        let todo = test_story(
            "US-F1-002",
            "todo",
            8,
            Some("2026-06-01T09:00:00+0200"),
            None,
        );
        let early_created_only = WebStory {
            created: Some("2026-03-30T00:00:00+0200".to_string()),
            activated: Some("2026-03-30T00:00:00+0200".to_string()),
            work_started: None,
            ..test_story("US-F2-001", "draft", 5, None, None)
        };

        let rows = build_burnup(&[done.clone(), todo, early_created_only], &[]);

        assert_eq!(
            rows,
            vec![
                BurnupPoint {
                    date: "2026-06-01".to_string(),
                    completed: 0,
                    scope: 0,
                },
                BurnupPoint {
                    date: "2026-06-03".to_string(),
                    completed: 5,
                    scope: 0,
                },
                BurnupPoint {
                    date: Local::now().date_naive().to_string(),
                    completed: 5,
                    scope: 0,
                },
            ]
        );
    }

    #[test]
    fn build_burnup_scope_steps_with_sprint_commitments() {
        let today = Local::now().date_naive();
        let sprint_zero_start = (today - Days::new(10)).to_string();
        let sprint_one_start = (today - Days::new(2)).to_string();
        let work_started = (today - Days::new(9)).to_string();
        let done = test_story(
            "US-F1-001",
            "done",
            5,
            Some(&format!("{work_started}T09:00:00+0200")),
            Some(&format!("{}T12:00:00+0200", (today - Days::new(7)))),
        );
        let next = test_story("US-F1-002", "todo", 8, None, None);

        let rows = build_burnup(
            &[done.clone(), next.clone()],
            &[
                test_sprint_with_start("closed", &sprint_zero_start, vec![done]),
                test_sprint_with_start("active", &sprint_one_start, vec![next]),
            ],
        );

        assert_eq!(
            rows.first(),
            Some(&BurnupPoint {
                date: work_started,
                completed: 0,
                scope: 5,
            })
        );
        assert!(
            rows.iter()
                .any(|row| row.date == sprint_one_start && row.scope == 13)
        );
        assert_eq!(
            rows.last().map(|row| row.date.clone()),
            Some(today.to_string())
        );
    }

    #[test]
    fn build_burnup_includes_past_sprint_end_dates_as_scope_anchors() {
        let today = Local::now().date_naive();
        let sprint_start = (today - Days::new(6)).to_string();
        let sprint_end = (today - Days::new(3)).to_string();
        let work_started = (today - Days::new(5)).to_string();
        let done = test_story(
            "US-F1-001",
            "done",
            5,
            Some(&format!("{work_started}T09:00:00+0200")),
            Some(&format!("{}T12:00:00+0200", (today - Days::new(4)))),
        );
        let sprint_story = done.clone();

        let rows = build_burnup(
            std::slice::from_ref(&done),
            &[test_sprint_with_dates(
                "closed",
                &sprint_start,
                &sprint_end,
                vec![sprint_story],
            )],
        );

        assert!(
            rows.iter()
                .any(|row| row.date == sprint_end && row.scope == 5)
        );
    }

    #[test]
    fn build_burndown_uses_active_sprint_story_progress() {
        let done = test_story(
            "US-F1-001",
            "done",
            5,
            Some("2026-06-01T09:00:00+0200"),
            Some("2026-06-03T12:00:00+0200"),
        );
        let todo = test_story(
            "US-F1-002",
            "todo",
            8,
            Some("2026-06-01T09:00:00+0200"),
            None,
        );
        let rows = build_burndown(&[test_sprint("active", vec![done, todo])]);

        assert_eq!(
            rows.first(),
            Some(&BurndownPoint {
                date: "2026-06-01".to_string(),
                remaining: 13,
                ideal: 13,
            })
        );
        assert!(
            rows.iter()
                .any(|row| row.date == "2026-06-03" && row.remaining == 8)
        );
    }
}
