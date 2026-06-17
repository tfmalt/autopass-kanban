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
    burndown: Vec<Value>,
    burnup: Vec<Value>,
    lead_time: Vec<LeadTimePoint>,
    velocity: Vec<VelocityPoint>,
    forecast: Forecast,
    progress: ProjectProgress,
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForecastThroughput {
    samples: Vec<i64>,
    average: f64,
    median: f64,
    observed_day_count: usize,
}

#[derive(Debug, Serialize)]
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

async fn api_team(State(state): State<Arc<AppState>>) -> Result<Json<Vec<String>>, ApiResponse> {
    Ok(Json(load_team(&state.repo_root)?))
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
    DashboardMetrics {
        burndown: Vec::new(),
        burnup: Vec::new(),
        lead_time: build_lead_time(&repo.stories),
        velocity: build_velocity(&repo.sprints),
        forecast: build_forecast(&repo.stories, &repo.sprints),
        progress: compute_progress(&repo.stories),
    }
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
    let today = Local::now().date_naive();
    let remaining_points = stories
        .iter()
        .filter(|story| story.status != "done" && story.status != "dropped")
        .map(|story| story.story_points.unwrap_or(0))
        .sum::<i64>();
    let done_points = stories
        .iter()
        .filter(|story| story.status == "done")
        .map(|story| story.story_points.unwrap_or(0))
        .sum::<i64>();
    let completed_sprints = sprints
        .iter()
        .filter(|sprint| sprint.status.as_deref() == Some("closed"))
        .count()
        .max(1) as i64;
    let average = done_points as f64 / completed_sprints as f64;
    let days_remaining = if remaining_points == 0 {
        Some(0)
    } else if average > 0.0 {
        Some((remaining_points as f64 / average).ceil() as i64 * 10)
    } else {
        None
    };
    let date = days_remaining
        .and_then(|days| today.checked_add_days(Days::new(days as u64)))
        .map(|date| date.to_string());
    Forecast {
        generated_at: Local::now().to_rfc3339(),
        remaining_points,
        sprint_duration_weeks: 2,
        projection_start_date: today.to_string(),
        throughput: ForecastThroughput {
            samples: Vec::new(),
            average,
            median: average,
            observed_day_count: 0,
        },
        completion: ForecastCompletion {
            p50_days: days_remaining,
            p80_days: days_remaining,
            p90_days: days_remaining,
            p50_date: date.clone(),
            p80_date: date.clone(),
            p90_date: date,
        },
        confidence: if average > 0.0 {
            "low".to_string()
        } else {
            "none".to_string()
        },
    }
}

fn load_team(repo_root: &Path) -> Result<Vec<String>> {
    let team_file = repo_root.join(".kanban/team.json");
    if let Ok(raw) = fs::read_to_string(&team_file)
        && let Ok(values) = serde_json::from_str::<Vec<BTreeMap<String, String>>>(&raw)
    {
        return Ok(values
            .into_iter()
            .filter_map(|mut item| {
                let name = item.remove("name")?;
                let email = item.remove("email")?;
                (!name.trim().is_empty() && !email.trim().is_empty())
                    .then(|| format!("{} <{}>", name.trim(), email.trim()))
            })
            .collect());
    }

    let repository = read_repository(repo_root)?;
    let mut seen = BTreeSet::new();
    for story in repository.stories {
        if let Some(assignee) = story.frontmatter.get("assignee") {
            for person in parse_assignees(assignee) {
                if person.contains('<')
                    && person.contains('@')
                    && !person.eq_ignore_ascii_case("Name <email@example.com>")
                {
                    seen.insert(person);
                }
            }
        }
    }
    Ok(seen.into_iter().collect())
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
    let start = NaiveDate::parse_from_str(start.get(..10)?, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(end.get(..10)?, "%Y-%m-%d").ok()?;
    Some((end - start).num_days().max(0))
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
}
