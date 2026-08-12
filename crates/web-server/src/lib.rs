use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, watch};

use kanban_core::*;

#[cfg(test)]
mod bench;
mod changes;
mod dto;
mod git;
mod handlers;
mod metrics;
mod pending;
mod read_model;
mod snapshot;
mod sprint_io;
mod static_assets;
mod team;
#[cfg(test)]
mod typegen;

use changes::ChangeBroadcaster;
use dto::ApiError;
use handlers::{
    api_config, api_create_sprint, api_epic, api_events, api_git_pull, api_git_push,
    api_git_status, api_metrics, api_move_story, api_plan_story, api_report, api_repository,
    api_story, api_team, api_team_avatar, api_update_epic_fields, api_update_sprint,
    api_update_story_body, api_update_story_fields, api_update_task, static_asset,
};

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
    branch_cache: Arc<Mutex<Option<String>>>,
    sse_subscribers: Arc<AtomicUsize>,
    /// Coalesces source-change signals into at most one identified `change`
    /// event per burst. Every mutation handler and the filesystem watcher must
    /// go through this rather than broadcasting directly (see `changes.rs`).
    changes: ChangeBroadcaster,
    /// Set to `true` once a shutdown signal arrives.
    ///
    /// `axum::serve(..).with_graceful_shutdown(..)` stops accepting new
    /// connections and then waits for the in-flight ones to finish. An SSE
    /// stream never finishes on its own, so without this the server survives
    /// SIGTERM for as long as any browser tab holds `/api/events` open, and
    /// `kanban web stop` falls through to SIGKILL after its timeout.
    shutdown: watch::Receiver<bool>,
    /// In-process write mutex that serializes web-server mutation handlers so
    /// concurrent UI actions cannot interleave read-modify-write sequences on
    /// the markdown source of truth (US-013). The core `RepoLock` provides the
    /// cross-process advisory lock; this mutex orders writes within the server.
    write_lock: Arc<Mutex<()>>,
    /// Guard against concurrent git-pull operations triggered from the web UI.
    pull_in_progress: Arc<AtomicBool>,
    /// Guard against concurrent git-push operations triggered from the web UI.
    push_in_progress: Arc<AtomicBool>,
    git: Option<git::GitContext>,
    pending: Option<pending::PendingStore>,
}

#[cfg(test)]
impl AppState {
    /// Test builder so tests do not hand-construct every field and drift from
    /// the production wiring as fields are added.
    ///
    /// Must be called from inside a Tokio runtime: `ChangeBroadcaster` spawns
    /// its coalescing task on construction.
    pub(crate) fn for_test(repo_root: PathBuf) -> Self {
        let (shutdown_tx, shutdown) = watch::channel(false);
        // Keep the sender alive for the lifetime of the state so `changed()`
        // parks instead of resolving immediately on a closed channel.
        std::mem::forget(shutdown_tx);
        let git = git::GitContext::discover(&repo_root);
        let pending = git.as_ref().map(|context| {
            pending::PendingStore::new(context.repo_root.clone(), context.pending_file())
        });
        Self {
            repo_root,
            host: "127.0.0.1".to_string(),
            port: 8080,
            branch_cache: Arc::new(Mutex::new(None)),
            sse_subscribers: Arc::new(AtomicUsize::new(0)),
            changes: ChangeBroadcaster::new(),
            shutdown,
            write_lock: Arc::new(Mutex::new(())),
            pull_in_progress: Arc::new(AtomicBool::new(false)),
            push_in_progress: Arc::new(AtomicBool::new(false)),
            git,
            pending,
        }
    }
}

pub fn serve_blocking(options: WebServeOptions) -> Result<()> {
    tokio::runtime::Runtime::new()
        .context("create kanban web runtime")?
        .block_on(serve(options))
}

/// Axum middleware that rejects cross-origin mutation requests (US-014).
///
/// Safe methods (`GET`, `HEAD`, `OPTIONS`) pass through unchanged. Every other
/// method must carry an `Origin` (falling back to `Referer`) whose authority
/// matches the server's bound address (`host:port`), otherwise the request is
/// rejected with `403 Forbidden` before any handler runs. This prevents a
/// malicious page visited in the same browser from driving mutation endpoints
/// against the local kanban server.
async fn csrf_guard(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    use axum::http::Method;

    let method = request.method().clone();
    if matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(request).await;
    }

    let bound_authority = format!("{}:{}", state.host, state.port);
    let allowed = request
        .headers()
        .get(axum::http::header::ORIGIN)
        .or_else(|| request.headers().get(axum::http::header::REFERER))
        .and_then(|value| value.to_str().ok())
        .and_then(authority_from_origin_or_referer)
        .is_some_and(|authority| authorities_match(&authority, &bound_authority));

    if allowed {
        next.run(request).await
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "cross-origin mutation blocked: Origin/Referer does not match the bound server address.".to_string(),
            }),
        )
            .into_response()
    }
}

/// Extract the `host:port` authority from an `Origin` or `Referer` header
/// value. Returns `None` when the value is not a valid URL with an authority.
fn authority_from_origin_or_referer(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let after_scheme = trimmed.split_once("://")?.1;
    let authority = after_scheme.split('/').next()?;
    if authority.is_empty() {
        None
    } else {
        Some(authority.to_string())
    }
}

fn authorities_match(candidate: &str, bound: &str) -> bool {
    candidate == bound
        || normalize_loopback_authority(candidate) == normalize_loopback_authority(bound)
}

fn normalize_loopback_authority(authority: &str) -> String {
    match authority.rsplit_once(':') {
        Some(("127.0.0.1" | "localhost", port)) => {
            format!("loopback:{port}")
        }
        _ => authority.to_string(),
    }
}

pub async fn serve(options: WebServeOptions) -> Result<()> {
    let config = load_kanban_config(&options.repo_root)?;
    let repo_root = config.repo_root;
    let changes = ChangeBroadcaster::new();
    let branch_cache = Arc::new(Mutex::new(None));
    let sse_subscribers = Arc::new(AtomicUsize::new(0));
    let _watcher = start_watcher(&repo_root, changes.clone(), branch_cache.clone())?;
    let (shutdown_tx, shutdown) = watch::channel(false);
    let git = git::GitContext::discover(&repo_root);
    let pending = git.as_ref().map(|context| {
        pending::PendingStore::new(context.repo_root.clone(), context.pending_file())
    });
    let state = Arc::new(AppState {
        repo_root,
        host: options.host,
        port: options.port,
        branch_cache,
        sse_subscribers,
        changes,
        shutdown,
        write_lock: Arc::new(Mutex::new(())),
        pull_in_progress: Arc::new(AtomicBool::new(false)),
        push_in_progress: Arc::new(AtomicBool::new(false)),
        git,
        pending,
    });
    let app = Router::new()
        .route("/api/repository", get(api_repository))
        .route("/api/metrics", get(api_metrics))
        .route("/api/report", get(api_report))
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
        .route("/api/git-pull", post(api_git_pull))
        .route("/api/git-push", post(api_git_push))
        .route("/api/git-status", get(api_git_status))
        .route("/api/events", get(api_events))
        .fallback(static_asset)
        .layer(middleware::from_fn_with_state(state.clone(), csrf_guard))
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
        .with_graceful_shutdown(shutdown_signal(shutdown_tx))
        .await
        .context("run kanban web server")
}

/// Resolve on SIGTERM or Ctrl-C, and tell open SSE streams to end first.
///
/// The ordering matters: axum only begins waiting for in-flight connections
/// after this future resolves, and an SSE stream would otherwise hold one open
/// indefinitely.
async fn shutdown_signal(shutdown: watch::Sender<bool>) {
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
    let _ = shutdown.send(true);
}

fn start_watcher(
    repo_root: &Path,
    changes: ChangeBroadcaster,
    branch_cache: Arc<Mutex<Option<String>>>,
) -> Result<RecommendedWatcher> {
    let config = load_kanban_config(repo_root)?;
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            // Branch invalidation stays on the raw event: it is a single
            // `Option` reset, and delaying it behind the debounce window would
            // serve a stale branch name for up to a second after a checkout.
            if let Ok(mut cache) = branch_cache.try_lock() {
                *cache = None;
            }
            changes.notify();
        }
    })?;
    if config.backlog_path().exists() {
        watcher.watch(&config.backlog_path(), RecursiveMode::Recursive)?;
    }
    if config.sprints_path().exists() {
        watcher.watch(&config.sprints_path(), RecursiveMode::Recursive)?;
    }
    // Configuration changes alter which files are served and how they are
    // interpreted, so `.kanban/settings.json` edits must invalidate too.
    let config_dir = config.repo_root.join(".kanban");
    if config_dir.exists() {
        watcher.watch(&config_dir, RecursiveMode::NonRecursive)?;
    }
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csrf_test_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/api/stories/{id}/move", post(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state.clone(), csrf_guard))
            .with_state(state)
    }

    fn csrf_test_state() -> Arc<AppState> {
        Arc::new(AppState::for_test(PathBuf::from(
            "/tmp/nonexistent-csrf-test",
        )))
    }

    #[tokio::test]
    async fn csrf_allows_same_origin_mutation() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/stories/US-001/move")
                    .header("Origin", "http://127.0.0.1:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_allows_localhost_origin_for_loopback_bound_server() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/stories/US-001/move")
                    .header("Origin", "http://localhost:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_rejects_cross_origin_mutation() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/stories/US-001/move")
                    .header("Origin", "http://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn csrf_rejects_mutation_with_missing_origin_and_referer() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/stories/US-001/move")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn csrf_allows_referer_fallback_when_origin_absent() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/stories/US-001/move")
                    .header("Referer", "http://127.0.0.1:8080/board")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_passes_get_requests_through_unchanged() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let app = csrf_test_router(csrf_test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/stories/US-001/move")
                    .header("Origin", "http://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn authority_from_origin_or_referer_extracts_host_port() {
        assert_eq!(
            authority_from_origin_or_referer("http://127.0.0.1:8080"),
            Some("127.0.0.1:8080".to_string())
        );
        assert_eq!(
            authority_from_origin_or_referer("http://127.0.0.1:8080/board"),
            Some("127.0.0.1:8080".to_string())
        );
        assert_eq!(
            authority_from_origin_or_referer("https://evil.example"),
            Some("evil.example".to_string())
        );
        assert_eq!(authority_from_origin_or_referer(""), None);
        assert_eq!(authority_from_origin_or_referer("not a url"), None);
    }

    #[test]
    fn authorities_match_treats_localhost_and_loopback_as_equivalent() {
        assert!(authorities_match("localhost:3000", "127.0.0.1:3000"));
        assert!(authorities_match("127.0.0.1:3000", "localhost:3000"));
        assert!(!authorities_match("localhost:3001", "127.0.0.1:3000"));
        assert!(!authorities_match("localhost:3000", "evil.example:3000"));
    }
}
