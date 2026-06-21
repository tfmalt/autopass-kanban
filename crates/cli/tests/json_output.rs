use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

fn init_backlog_and_sprints(dir: &Path) {
    init_repo(dir);
    fs::create_dir_all(dir.join("delivery/backlog")).expect("create backlog dir");
    fs::create_dir_all(dir.join("delivery/sprints")).expect("create sprints dir");
}

fn write_sprint(root: &Path, name: &str, headline: &str) {
    let sprints_dir = root.join("delivery/sprints");
    fs::create_dir_all(&sprints_dir).expect("create sprints dir");
    let file_name = format!("{name}.{headline}.md");
    let path = sprints_dir.join(&file_name);
    let content = format!(
        "---\nsprint: {name}\nheadline: {headline}\nstart_date: 2026-06-01\nend_date: 2026-06-12\nstatus: active\nwip_limit: ~\n---\n\n# {name}: {headline}\n\n## Sprint Goal\n\nBuild the foundation.\n"
    );
    fs::write(&path, content).expect("write sprint file");
}

fn kanban_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn write_story(root: &Path, rel: &str, frontmatter: &str, body: &str) {
    let full_path = root.join(rel);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("create story dir");
    }
    let content = format!("---\n{frontmatter}---\n\n{body}\n");
    fs::write(&full_path, content).expect("write story file");
}

fn init_repo(dir: &std::path::Path) {
    let repo_root = dir.to_string_lossy().into_owned();
    let output = kanban_in(dir, &["init", &repo_root]);
    assert!(
        output.status.success(),
        "kanban init should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn parse_stdout(out: &Output) -> serde_json::Value {
    let stdout = String::from_utf8(out.stdout.clone()).expect("stdout should be utf8");
    serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!("stdout should parse as JSON; error: {err}\nraw stdout:\n{stdout}")
    })
}

#[test]
fn config_get_emits_ok_envelope() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    let out = kanban_in(
        dir.path(),
        &[
            "--format",
            "json",
            "config",
            "get",
            "paths.backlog",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(
        json["kind"], "config.get",
        "envelope kind should be config.get"
    );
    assert_eq!(json["schema_version"], 1, "schema_version should be 1");
    assert_eq!(
        json["data"]["key"], "paths.backlog",
        "data.key should match requested key"
    );
    // Accept whatever default value `init` writes for paths.backlog
    assert!(
        json["data"]["value"].is_string(),
        "data.value should be a string; got: {}",
        json["data"]["value"]
    );
}

#[test]
fn init_emits_ok_envelope() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    let out = kanban_in(dir.path(), &["--format", "json", "init", &repo_root]);

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0 for init JSON; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "init", "envelope kind should be init");
    assert_eq!(
        json["data"]["created_count"], 4,
        "init should create the four default config files"
    );
}

#[test]
fn init_with_no_sprints_persists_feature_flag() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "init", "--no-sprints", &repo_root],
    );
    assert!(
        out.status.success(),
        "init --no-sprints should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let paths_file = dir.path().join(".kanban/paths.json");
    let json = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(&paths_file).expect("paths.json should be written"),
    )
    .expect("paths.json should parse as json");
    assert_eq!(json["features"]["sprints"], false);
    assert_eq!(json["features"]["phases"], true);
    assert_eq!(json["features"]["epics"], true);
}

#[test]
fn init_with_no_epics_and_no_phases_persists_feature_flags() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    let out = kanban_in(
        dir.path(),
        &[
            "--format",
            "json",
            "init",
            "--no-epics",
            "--no-phases",
            &repo_root,
        ],
    );
    assert!(
        out.status.success(),
        "init --no-epics --no-phases should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let paths_file = dir.path().join(".kanban/paths.json");
    let json = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(&paths_file).expect("paths.json should be written"),
    )
    .expect("paths.json should parse as json");
    assert_eq!(json["features"]["sprints"], true);
    assert_eq!(json["features"]["phases"], false);
    assert_eq!(json["features"]["epics"], false);
}

#[test]
fn features_list_json_emits_feature_state() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    kanban_in(dir.path(), &["init", "--no-sprints", &repo_root]);

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "features", "list", &repo_root],
    );
    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "features.list");
    assert_eq!(json["data"]["sprints"], false);
    assert_eq!(json["data"]["phases"], true);
    assert_eq!(json["data"]["epics"], true);
}

#[test]
fn config_get_unknown_key_emits_config_key_not_found_error() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    let out = kanban_in(
        dir.path(),
        &[
            "--format",
            "json",
            "config",
            "get",
            "no.such.key",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "exit code should be 1 for unknown config key; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "error", "envelope status should be error");
    assert_eq!(
        json["kind"], "config.get",
        "envelope kind should be config.get even on error"
    );
    assert_eq!(
        json["error"]["code"], "config_key_not_found",
        "error code should be config_key_not_found"
    );
}

#[test]
fn story_show_emits_story_with_normalized_status() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    // Write story under the default backlog path that `kanban init` configures:
    // delivery/backlog  (DEFAULT_BACKLOG_PATH in config.rs)
    let rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    let frontmatter = "id: US-F1-001\nstatus: In Progress\nstory_points: 3\nsprint: S001\ntype: story\nepic: EP-F1-01\nwork_started: ~\nwork_done: ~\ncreated: 2026-01-01T00:00:00+02:00\nupdated: 2026-01-01T00:00:00+02:00\n";
    let body = "# User Story: Cluster\n\n## Acceptance Criteria\n\nGiven a cluster exists, when something happens, then it works.\n";
    write_story(dir.path(), rel, frontmatter, body);

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "story", "show", "US-F1-001", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(
        json["kind"], "story.show",
        "envelope kind should be story.show"
    );
    assert_eq!(json["data"]["id"], "US-F1-001");
    assert_eq!(
        json["data"]["status_normalized"], "in-progress",
        "status_normalized should be in-progress"
    );
    assert_eq!(json["data"]["story_points"], 3, "story_points should be 3");
    assert!(
        json["data"]["frontmatter"].is_object(),
        "frontmatter should be an object; got: {}",
        json["data"]["frontmatter"]
    );
    assert!(
        json["data"]["body"]
            .as_str()
            .unwrap_or("")
            .contains("Acceptance Criteria"),
        "body should contain 'Acceptance Criteria'; got: {}",
        json["data"]["body"]
    );
}

#[test]
fn story_show_missing_id_emits_story_not_found() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());
    // Create the backlog directory so read_repository succeeds and returns Ok(None)
    // rather than an IO error from WalkDir when the directory doesn't exist.
    fs::create_dir_all(dir.path().join("delivery/backlog")).expect("create backlog dir");

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "story", "show", "US-F1-999", &repo_root],
    );

    let json = parse_stdout(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "exit code should be 1 for missing story; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "error", "envelope status should be error");
    assert_eq!(
        json["error"]["code"], "story_not_found",
        "error code should be story_not_found"
    );
}

#[test]
fn epic_show_emits_epic_with_progress_sections_and_child_stories() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    let epic_rel =
        "delivery/backlog/phase-1/06.tooling/EP-F1-06-git-driven-kanban-and-backlog-tooling.md";
    let epic_frontmatter = "id: EP-F1-06\ntype: epic\nstatus: draft\nphase: 1\nowner: Solution Architect / Product Owner\nmilestone: MP1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-06-11T14:08:39+0200\n";
    let epic_body = "# Epic: Git-driven kanban and backlog tooling\n\n## Business Context\n\nMarkdown-first workflow.\n\n## Acceptance Criteria\n\n- Epic inspection is available.\n";
    write_story(dir.path(), epic_rel, epic_frontmatter, epic_body);

    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md";
    let story_frontmatter = "id: US-F1-052\nstatus: done\nstory_points: 5\nsprint: S001\ntype: user-story\nepic: EP-F1-06\nassignee: TBD\nwork_started: ~\nwork_done: 2026-01-01T00:00:00+02:00\ncreated: 2026-01-01T00:00:00+02:00\nupdated: 2026-01-01T00:00:00+02:00\n";
    let story_body = "# User Story: Add read-only CLI for sprint and backlog inspection\n";
    write_story(dir.path(), story_rel, story_frontmatter, story_body);

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "epic", "show", "EP-F1-06", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "epic.show");
    assert_eq!(json["data"]["id"], "EP-F1-06");
    assert_eq!(json["data"]["phase"], "1");
    assert_eq!(json["data"]["story_ids"][0], "US-F1-052");
    assert_eq!(
        json["data"]["stories_by_status"]["done"][0]["id"],
        "US-F1-052"
    );
    assert_eq!(
        json["data"]["sections"]["business_context"],
        "Markdown-first workflow."
    );
}

#[test]
fn epic_show_missing_id_emits_epic_not_found() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());
    fs::create_dir_all(dir.path().join("delivery/backlog")).expect("create backlog dir");

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "epic", "show", "EP-F1-999", &repo_root],
    );

    let json = parse_stdout(&out);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "epic_not_found");
}

#[test]
fn sprint_list_emits_array_with_is_current() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    // Ensure delivery/backlog exists (required by read_repository) and write a sprint fixture.
    fs::create_dir_all(dir.path().join("delivery/backlog")).expect("create backlog dir");
    fs::create_dir_all(dir.path().join("delivery/sprints")).expect("create sprints dir");
    write_sprint(dir.path(), "S001", "foundation");

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "sprint", "list", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(
        json["kind"], "sprint.list",
        "envelope kind should be sprint.list"
    );
    assert_eq!(
        json["data"]["count"], 1,
        "data.count should be 1; got: {}",
        json["data"]["count"]
    );
    assert!(
        json["data"]["sprints"].is_array(),
        "data.sprints should be an array"
    );
    assert_eq!(
        json["data"]["sprints"][0]["sprint_name"], "S001.foundation",
        "first sprint name should be S001.foundation; got: {}",
        json["data"]["sprints"][0]["sprint_name"]
    );
    assert_eq!(
        json["data"]["sprints"][0]["is_current"], true,
        "is_current should be true when today (2026-06-03) falls within the sprint's date range (2026-06-01..2026-06-12)"
    );
}

#[test]
fn validate_clean_repo_is_ok() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_backlog_and_sprints(dir.path());

    let out = kanban_in(dir.path(), &["--format", "json", "validate", &repo_root]);

    let json = parse_stdout(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "exit code should be 0 for a clean repo; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "validate", "envelope kind should be validate");
    assert_eq!(json["schema_version"], 1, "schema_version should be 1");
    assert_eq!(
        json["data"]["valid"], true,
        "data.valid should be true for a clean repo"
    );
    assert_eq!(
        json["data"]["issue_count"], 0,
        "data.issue_count should be 0 for a clean repo"
    );
    assert_eq!(
        json["data"]["story_count"], 0,
        "data.story_count should be 0 for a freshly-initialized empty repo"
    );
    assert!(
        json["data"]["issues"]
            .as_array()
            .is_some_and(|a| a.is_empty()),
        "data.issues should be an empty array"
    );
}

#[test]
fn config_show_nests_paths() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "config", "show", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "exit code should be 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(
        json["kind"], "config.show",
        "envelope kind should be config.show"
    );
    assert_eq!(json["schema_version"], 1, "schema_version should be 1");
    assert!(
        json["data"].is_object(),
        "data should be a JSON object; got: {}",
        json["data"]
    );
    assert!(
        json["data"]["paths"].is_object(),
        "data.paths should be a JSON object; got: {}",
        json["data"]["paths"]
    );
    assert!(
        json["data"]["paths"]["backlog"].is_string(),
        "data.paths.backlog should be a string; got: {}",
        json["data"]["paths"]["backlog"]
    );
}

#[test]
fn config_set_emits_updated_value() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_repo(dir.path());

    let out = kanban_in(
        dir.path(),
        &[
            "--format",
            "json",
            "config",
            "set",
            "web.host",
            "127.0.0.1",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "config set should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "config.set", "kind should be config.set");
    assert_eq!(json["data"]["key"], "web.host");
    assert_eq!(json["data"]["value"], "127.0.0.1");
    assert!(
        json["data"]["file_path"]
            .as_str()
            .is_some_and(|path| path.ends_with(".kanban/web.json")),
        "file_path should point at web.json; got: {}",
        json["data"]["file_path"]
    );
}

/// Write a story file with the given frontmatter and body for tests needing
/// a story that can be moved (must have a valid sprint assignment).
fn write_story_in_sprint(root: &Path, rel: &str, story_id: &str, sprint_name: &str, status: &str) {
    let frontmatter = format!(
        "id: {story_id}\ntype: user-story\nstatus: {status}\nepic: EP-F1-01\nsprint: {sprint_name}\nassignee: ~\nstory_points: 3\nwork_started: ~\nwork_done: ~\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n"
    );
    write_story(root, rel, &frontmatter, "# User Story\n\nBody.\n");
}

#[test]
fn story_move_emits_result_with_normalized_statuses() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    write_sprint(root, "S001", "foundation");

    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    write_story_in_sprint(root, story_rel, "US-F1-001", "S001.foundation", "todo");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "story",
            "move",
            "US-F1-001",
            "in-progress",
            "--assignee",
            "Tester <t@x.no>",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);

    // Should succeed (ok path) — verify the JSON shape
    assert_eq!(
        json["status"],
        "ok",
        "envelope status should be ok; stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8(out.stdout.clone()).unwrap_or_default()
    );
    assert_eq!(json["kind"], "story.move", "kind should be story.move");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["data"]["story_id"], "US-F1-001");
    assert_eq!(
        json["data"]["to_status_normalized"], "in-progress",
        "to_status_normalized should be in-progress"
    );
    assert!(
        json["data"]["story_path"]
            .as_str()
            .is_some_and(|p| p.contains("US-F1-001")),
        "story_path should contain story id; got: {}",
        json["data"]["story_path"]
    );
    assert!(
        json["error"].is_null(),
        "error should be null on success; got: {}",
        json["error"]
    );
    assert_eq!(out.status.code(), Some(0), "exit code should be 0");
}

#[test]
fn story_rm_alias_deletes_story_and_task_file() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    write_sprint(root, "S001", "foundation");

    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    write_story_in_sprint(root, story_rel, "US-F1-001", "S001.foundation", "todo");
    let story_path = root.join(story_rel);
    let task_path = story_path.with_extension("tasks.md");
    fs::write(
        &task_path,
        "# Tasks for US-F1-001\n\nParent User Story: US-F1-001\nSprint: S001.foundation\n",
    )
    .expect("write task file");

    let out = kanban_in(
        root,
        &["--format", "json", "story", "rm", "US-F1-001", &repo_root],
    );
    let json = parse_stdout(&out);

    assert_eq!(
        json["status"],
        "ok",
        "story rm should be ok; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["kind"], "story.delete");
    assert_eq!(json["data"]["story_id"], "US-F1-001");
    assert_eq!(json["data"]["sprint_name"], "S001.foundation");
    assert!(
        json["data"]["task_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("US-F1-001-cluster.tasks.md")),
        "task_path should point at deleted task file; got: {}",
        json["data"]["task_path"]
    );
    assert!(!story_path.exists());
    assert!(!task_path.exists());
    assert_eq!(out.status.code(), Some(0), "exit code should be 0");
}

#[test]
fn sprint_sync_emits_changed_list() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    write_sprint(root, "S001", "foundation");

    let out = kanban_in(root, &["--format", "json", "sprint", "sync", &repo_root]);

    let json = parse_stdout(&out);
    assert_eq!(
        json["status"],
        "ok",
        "sprint sync should be ok; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["kind"], "sprint.sync", "kind should be sprint.sync");
    assert_eq!(json["schema_version"], 1);
    assert!(
        json["data"]["count"].is_number(),
        "data.count should be a number; got: {}",
        json["data"]["count"]
    );
    assert!(
        json["data"]["changed_sprints"].is_array(),
        "data.changed_sprints should be an array; got: {}",
        json["data"]["changed_sprints"]
    );
    assert_eq!(out.status.code(), Some(0), "exit code should be 0");
}

#[test]
fn task_add_emits_result_with_task_body() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    write_sprint(root, "S001", "foundation");

    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    write_story_in_sprint(
        root,
        story_rel,
        "US-F1-001",
        "S001.foundation",
        "in-progress",
    );

    // Create a task file so the story is recognized as a sprint story with tasks
    let task_file = root.join("delivery/backlog/phase-1/01.infra/US-F1-001-cluster.tasks.md");
    fs::write(
        &task_file,
        "# Tasks for US-F1-001\n\nParent User Story: US-F1-001\nSprint: S001.foundation\n",
    )
    .expect("write task file");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "task",
            "add",
            "US-F1-001",
            "--title",
            "Setup cluster",
            "--description",
            "Install k8s cluster on dev.",
            "--status",
            "todo",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(
        json["status"],
        "ok",
        "task add should be ok; stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8(out.stdout.clone()).unwrap_or_default()
    );
    assert_eq!(json["kind"], "task.add", "kind should be task.add");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["data"]["story_id"], "US-F1-001");
    assert!(
        json["data"]["task_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("TASK-US-F1-001-")),
        "task_id should start with TASK-US-F1-001-; got: {}",
        json["data"]["task_id"]
    );
    assert_eq!(json["data"]["task"]["title"], "Setup cluster");
    assert_eq!(json["data"]["task"]["status_normalized"], "todo");
    assert!(
        json["data"]["task_path"]
            .as_str()
            .is_some_and(|p| p.contains("US-F1-001")),
        "task_path should reference the story; got: {}",
        json["data"]["task_path"]
    );
    assert_eq!(out.status.code(), Some(0), "exit code should be 0");
}

#[test]
fn task_show_emits_task_list_for_story() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.md";
    let frontmatter = "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n";
    write_story(root, story_rel, frontmatter, "# User Story\n\nBody.\n");
    let task_file = root.join("delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.tasks.md");
    fs::write(
        &task_file,
        "# Tasks for US-F1-057\n\nParent User Story: US-F1-057\nSprint: ~\n\n## TASK-US-F1-057-001 - Read tasks\n\nStatus: todo\nTags: cli\n\nDescription:\nRead tasks.\n",
    )
    .expect("write task file");

    let out = kanban_in(
        root,
        &["--format", "json", "task", "show", "US-F1-057", &repo_root],
    );

    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "task.show");
    assert_eq!(json["data"]["story_id"], "US-F1-057");
    assert_eq!(json["data"]["task_count"], 1);
    assert_eq!(json["data"]["tasks"][0]["id"], "TASK-US-F1-057-001");
}

#[test]
fn task_delete_removes_task_from_story() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.md";
    let frontmatter = "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n";
    write_story(root, story_rel, frontmatter, "# User Story\n\nBody.\n");
    let task_file = root.join("delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.tasks.md");
    fs::write(
        &task_file,
        "# Tasks for US-F1-057\n\nParent User Story: US-F1-057\nSprint: ~\n\n## TASK-US-F1-057-001 - First task\n\nStatus: todo\nTags: cli\n\nDescription:\nFirst.\n\n## TASK-US-F1-057-002 - Second task\n\nStatus: todo\nTags: tests\n\nDescription:\nSecond.\n",
    )
    .expect("write task file");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "task",
            "delete",
            "US-F1-057",
            "TASK-US-F1-057-001",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    let updated = fs::read_to_string(task_file).expect("task file should be readable");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "task.delete");
    assert_eq!(json["data"]["task_id"], "TASK-US-F1-057-001");
    assert!(!updated.contains("TASK-US-F1-057-001 - First task"));
    assert!(updated.contains("TASK-US-F1-057-002 - Second task"));
}

#[test]
fn task_delete_missing_task_emits_error_without_partial_write() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.md";
    let frontmatter = "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n";
    write_story(root, story_rel, frontmatter, "# User Story\n\nBody.\n");
    let task_file = root.join("delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.tasks.md");
    let original = "# Tasks for US-F1-057\n\nParent User Story: US-F1-057\nSprint: ~\n\n## TASK-US-F1-057-001 - First task\n\nStatus: todo\nTags: cli\n\nDescription:\nFirst.\n";
    fs::write(&task_file, original).expect("write task file");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "task",
            "delete",
            "US-F1-057",
            "TASK-US-F1-057-999",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    let updated = fs::read_to_string(task_file).expect("task file should be readable");
    assert_eq!(json["status"], "error");
    assert_eq!(json["kind"], "task.delete");
    assert_eq!(json["error"]["code"], "internal");
    assert_eq!(updated, original);
}

#[test]
fn task_add_accepts_backlog_story_without_sprint_assignment() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.md";
    let frontmatter = "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n";
    write_story(root, story_rel, frontmatter, "# User Story\n\nBody.\n");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "task",
            "add",
            "US-F1-057",
            "--title",
            "Plan task",
            "--description",
            "Plan work before sprint assignment.",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(
        json["status"],
        "ok",
        "task add should be ok for backlog stories; stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8(out.stdout.clone()).unwrap_or_default()
    );
    assert_eq!(json["kind"], "task.add");
    assert_eq!(json["data"]["story_id"], "US-F1-057");
    assert_eq!(json["data"]["task_id"], "TASK-US-F1-057-001");
    let task_file = root.join("delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.tasks.md");
    let task_markdown = fs::read_to_string(task_file).expect("task file should be created");
    assert!(task_markdown.contains("Sprint: ~"));
    assert!(task_markdown.contains("## TASK-US-F1-057-001 - Plan task"));
    assert_eq!(out.status.code(), Some(0), "exit code should be 0");
}

#[test]
fn task_add_twice_does_not_emit_double_separator() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    let story_rel = "delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.md";
    let frontmatter = "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n";
    write_story(root, story_rel, frontmatter, "# User Story\n\nBody.\n");

    let first = kanban_in(
        root,
        &[
            "task",
            "add",
            "US-F1-057",
            "--title",
            "A new task",
            "--description",
            "add another task to the user story",
            &repo_root,
        ],
    );
    assert!(
        first.status.success(),
        "first task add should succeed; stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let second = kanban_in(
        root,
        &[
            "task",
            "add",
            "US-F1-057",
            "--title",
            "A new task 2",
            "--description",
            "add another new task to the user story",
            &repo_root,
        ],
    );
    assert!(
        second.status.success(),
        "second task add should succeed; stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let task_file = root.join("delivery/backlog/phase-1/06.tooling/US-F1-057-task-crud.tasks.md");
    let task_markdown = fs::read_to_string(task_file).expect("task file should be created");
    assert!(!task_markdown.contains("\n\n---\n\n---\n\n"));
    assert!(task_markdown.contains("## TASK-US-F1-057-001 - A new task"));
    assert!(task_markdown.contains("## TASK-US-F1-057-002 - A new task 2"));
}

#[test]
fn sprint_create_without_headline_emits_invalid_argument() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    // Call sprint create in JSON mode without --headline or --non-interactive
    let out = kanban_in(root, &["--format", "json", "sprint", "create", &repo_root]);

    let json = parse_stdout(&out);
    assert_eq!(
        json["status"],
        "error",
        "should be error without --headline in JSON mode; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["kind"], "sprint.create");
    assert_eq!(json["error"]["code"], "invalid_argument");
    assert_eq!(out.status.code(), Some(1), "exit code should be 1");
}

#[test]
fn doctor_show_emits_kind_and_status() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();

    init_backlog_and_sprints(dir.path());

    let out = kanban_in(
        dir.path(),
        &["--format", "json", "doctor", "show", &repo_root],
    );

    let json = parse_stdout(&out);
    // A freshly init'd repo may be healthy or have warnings (no active sprint),
    // but must never produce an error status.
    assert_ne!(
        json["status"],
        "error",
        "doctor show should never emit an error envelope on a well-formed repo; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["kind"], "doctor", "envelope kind should be doctor");
    assert_eq!(json["schema_version"], 1, "schema_version should be 1");
    assert!(
        json["data"]["findings"].is_array(),
        "data.findings should be an array"
    );
    assert!(
        json["data"]["summary"].is_object(),
        "data.summary should be an object"
    );
    assert_eq!(
        json["data"]["summary"]["error"], 0,
        "a fresh repo should have no error-severity findings"
    );
    // Exit code must be consistent with status
    let expected_exit = if json["status"] == "ok" { 0 } else { 2 };
    assert_eq!(
        out.status.code(),
        Some(expected_exit),
        "exit code should match envelope status ({} => {}); stderr: {}",
        json["status"],
        expected_exit,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sprint_create_with_flag_but_no_headline_emits_invalid_argument() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    // --number is supplied but --headline is omitted; headline is required in
    // non-interactive JSON mode so this must fail with invalid_argument.
    let out = kanban_in(
        root,
        &[
            "--format", "json", "sprint", "create", "--number", "5", &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(
        json["status"],
        "error",
        "should be error when --number is given but --headline is absent; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["kind"], "sprint.create");
    assert_eq!(
        json["error"]["code"], "invalid_argument",
        "error code should be invalid_argument; got: {}",
        json["error"]["code"]
    );
    assert_eq!(out.status.code(), Some(1), "exit code should be 1");
}

#[test]
fn json_mode_stdout_is_exactly_one_document_no_ansi() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    init_repo(root);
    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "config",
            "get",
            "paths.backlog",
            root.to_str().unwrap(),
        ],
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.contains('\u{1b}'), "stdout contained ANSI escapes");
    let trimmed = stdout.trim();
    let _: serde_json::Value = serde_json::from_str(trimmed).expect("single JSON doc");
    assert!(
        trimmed.starts_with('{'),
        "stdout did not start with JSON: {trimmed:?}"
    );
}

#[test]
fn json_mode_warning_stdout_is_single_document() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    // Build a repo with backlog + sprints dirs so validate can run without IO errors.
    init_backlog_and_sprints(root);

    let out = kanban_in(
        root,
        &["--format", "json", "validate", root.to_str().unwrap()],
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.contains('\u{1b}'), "stdout contained ANSI escapes");
    let trimmed = stdout.trim();
    let json: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be a single parseable JSON doc");
    assert!(
        trimmed.starts_with('{'),
        "stdout did not start with JSON object: {trimmed:?}"
    );
    assert_eq!(
        json["kind"], "validate",
        "envelope kind must be 'validate'; got: {}",
        json["kind"]
    );
}

// ── Fix 1 contract tests: story.list scope labels ─────────────────────────────

#[test]
fn story_list_default_scope_is_current() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    // Write an active sprint so list_current_sprint_stories has a valid current sprint.
    write_sprint(root, "S001", "foundation");

    let out = kanban_in(root, &["--format", "json", "story", "list", &repo_root]);

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "story list should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        json["kind"], "story.list",
        "envelope kind should be story.list"
    );
    assert_eq!(
        json["data"]["scope"], "current",
        "default scope should be 'current', not a sprint name; got: {}",
        json["data"]["scope"]
    );
}

#[test]
fn story_list_all_scope_is_all() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    let out = kanban_in(
        root,
        &["--format", "json", "story", "list", "--all", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "story list --all should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        json["kind"], "story.list",
        "envelope kind should be story.list"
    );
    assert_eq!(
        json["data"]["scope"], "all",
        "scope for --all should be 'all'; got: {}",
        json["data"]["scope"]
    );
}

// ── Test 3: sprint current emits overview ─────────────────────────────────────

#[test]
fn sprint_current_emits_overview() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    // Dates 2026-06-01..2026-06-12 include today (2026-06-03), so this is current.
    write_sprint(root, "S001", "foundation");

    let out = kanban_in(root, &["--format", "json", "sprint", "current", &repo_root]);

    let json = parse_stdout(&out);
    // sprint current should succeed because the date range covers 2026-06-03.
    assert!(
        out.status.success(),
        "sprint current should succeed when a sprint covers today; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(
        json["kind"], "sprint.current",
        "envelope kind should be sprint.current"
    );
    assert!(
        json["data"]["sprint_name"].is_string(),
        "data.sprint_name should be a string; got: {}",
        json["data"]["sprint_name"]
    );
}

// ── Test 4: phase show emits stories ──────────────────────────────────────────

#[test]
fn phase_show_emits_stories() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    // summarize_phase looks for paths matching /delivery/backlog/phase-1-<anything>/...
    // The path must contain the backlog_marker (/delivery/backlog/) + "phase-1-".
    let rel = "delivery/backlog/phase-1-scaffolding/01.infra/US-F1-001-cluster.md";
    let frontmatter = "id: US-F1-001\ntype: user-story\nstatus: todo\nepic: EP-F1-01\nsprint: ~\nassignee: ~\nstory_points: 3\nwork_started: ~\nwork_done: ~\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n";
    write_story(root, rel, frontmatter, "# User Story\n\nBody.\n");

    // Phase argument is "F1" or "1" — normalize_phase_input strips non-digits then "F1" -> "1".
    let out = kanban_in(
        root,
        &["--format", "json", "phase", "show", "F1", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "phase show F1 should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        json["kind"], "phase.show",
        "envelope kind should be phase.show"
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert!(
        json["data"]["count"].is_number(),
        "data.count should be a number; got: {}",
        json["data"]["count"]
    );
    assert!(
        json["data"]["count"].as_u64().unwrap_or(0) >= 1,
        "data.count should be at least 1 after writing a phase-1 story; got: {}",
        json["data"]["count"]
    );
}

// ── Fix 2 contract test: doctor fix emits doctor.fix error envelope ───────────

#[test]
fn doctor_fix_json_emits_invalid_argument() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);

    // `doctor fix` has an optional `target` argument; pass only the repo root.
    // The explicit JSON arm should fire before the `_other` fallback.
    let out = kanban_in(root, &["--format", "json", "doctor", "fix", &repo_root]);

    let json = parse_stdout(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "exit code should be 1 for doctor fix in JSON mode; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "error", "envelope status should be error");
    assert_eq!(
        json["kind"], "doctor.fix",
        "envelope kind should be doctor.fix; got: {}",
        json["kind"]
    );
    assert_eq!(
        json["error"]["code"], "invalid_argument",
        "error code should be invalid_argument; got: {}",
        json["error"]["code"]
    );
}

#[test]
fn story_update_without_fields_json_emits_specific_error() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    write_story_in_sprint(root, story_rel, "US-F1-001", "S001.foundation", "todo");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "story",
            "update",
            "US-F1-001",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(out.status.code(), Some(1), "exit code should be 1");
    assert_eq!(json["status"], "error", "envelope status should be error");
    assert_eq!(json["kind"], "story.update", "kind should be story.update");
    assert_eq!(json["error"]["code"], "invalid_argument");
    assert!(
        json["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("editor mode is unavailable")),
        "message should explain editor mode is unavailable; got: {}",
        json["error"]["message"]
    );
}

#[test]
fn story_update_with_field_emits_updated_fields() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-001-cluster.md";
    write_story_in_sprint(root, story_rel, "US-F1-001", "S001.foundation", "todo");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "story",
            "update",
            "US-F1-001",
            "--assignee",
            "Tester <tester@example.no>",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "story update should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "story.update", "kind should be story.update");
    assert_eq!(json["data"]["story_id"], "US-F1-001");
    assert!(
        json["data"]["updated_fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field == "assignee")),
        "updated_fields should contain assignee; got: {}",
        json["data"]["updated_fields"]
    );
}

#[test]
fn epic_update_priority_json_output() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let epic_rel = "delivery/backlog/phase-1/06.tooling/EP-F1-02-priority-test.md";
    let frontmatter = "id: EP-F1-02\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n";
    write_story(root, epic_rel, frontmatter, "# Epic: Priority test\n");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "epic",
            "update",
            "EP-F1-02",
            "--priority",
            "10",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "epic update should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "epic.update");
    let markdown = fs::read_to_string(root.join(epic_rel)).expect("epic file should be readable");
    assert!(markdown.contains("priority: 10"));
}

#[test]
fn story_update_priority_json_output() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let story_rel = "delivery/backlog/phase-1/01.infra/US-F1-005-priority-test.md";
    let frontmatter = "id: US-F1-005\ntype: user-story\nstatus: todo\nepic: EP-F1-01\nsprint: ~\nassignee: TBD\nstory_points: 3\nwork_started: ~\nwork_done: ~\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n";
    write_story(
        root,
        story_rel,
        frontmatter,
        "# User Story: Priority test\n",
    );

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "story",
            "update",
            "US-F1-005",
            "--priority",
            "20",
            &repo_root,
        ],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "story update should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(json["kind"], "story.update");
    let markdown = fs::read_to_string(root.join(story_rel)).expect("story file should be readable");
    assert!(markdown.contains("priority: 20"));
}

#[test]
fn epic_update_priority_json_rejects_missing_value() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();

    init_backlog_and_sprints(root);
    let epic_rel = "delivery/backlog/phase-1/06.tooling/EP-F1-02-priority-missing-value.md";
    let frontmatter = "id: EP-F1-02\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n";
    write_story(root, epic_rel, frontmatter, "# Epic: Priority test\n");

    let out = kanban_in(
        root,
        &[
            "--format",
            "json",
            "epic",
            "update",
            "EP-F1-02",
            "--priority",
        ],
    );

    let json = parse_stdout(&out);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json["status"], "error");
    assert_eq!(json["kind"], "epic.update");
    assert_eq!(json["error"]["code"], "invalid_argument");
}

#[test]
fn completion_help_json_emits_help_content() {
    let dir = tempdir().expect("temp dir should be created");

    let out = kanban_in(dir.path(), &["--format", "json", "completion", "help"]);

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "completion help should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "completion", "kind should be completion");
    assert_eq!(json["data"]["target"], "help");
    assert_eq!(json["data"]["content_type"], "help");
    assert!(
        json["data"]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Generate a shell completion script")),
        "content should include completion help; got: {}",
        json["data"]["content"]
    );
}

#[test]
fn list_ids_json_emits_items() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_backlog_and_sprints(root);
    let rel = "delivery/backlog/phase-1-scaffolding/01.infra/US-F1-001-cluster.md";
    let frontmatter = "id: US-F1-001\ntype: user-story\nstatus: todo\nepic: EP-F1-01\nsprint: ~\nassignee: ~\nstory_points: 3\nwork_started: ~\nwork_done: ~\ncreated: 2026-01-01T00:00:00+01:00\nupdated: 2026-01-01T00:00:00+01:00\n";
    write_story(root, rel, frontmatter, "# User Story: Cluster\n\nBody.\n");

    let out = kanban_in(
        root,
        &["--format", "json", "list-ids", "stories", &repo_root],
    );

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "list-ids should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "list-ids", "kind should be list-ids");
    assert_eq!(json["data"]["kind"], "stories");
    assert!(
        json["data"]["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["value"] == "US-F1-001")),
        "items should include US-F1-001; got: {}",
        json["data"]["items"]
    );
}

#[test]
fn web_status_json_emits_state_without_starting_server() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_repo(root);

    let out = kanban_in(root, &["--format", "json", "web", "status", &repo_root]);

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "web status should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["status"], "ok", "envelope status should be ok");
    assert_eq!(json["kind"], "web.status", "kind should be web.status");
    assert_eq!(json["data"]["state"], "stopped");
    assert!(json["data"]["url"].is_string(), "url should be present");
}

#[test]
fn web_status_reports_recorded_running_port() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_repo(root);
    let run_dir = root.join(".kanban/run");
    fs::create_dir_all(&run_dir).expect("create run dir");
    fs::write(run_dir.join("web.pid"), format!("{}\n", std::process::id()))
        .expect("write pid file");
    fs::write(run_dir.join("web.port"), "3001\n").expect("write port file");

    let out = kanban_in(root, &["web", "status", &repo_root]);

    assert!(
        out.status.success(),
        "web status should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(":3001"),
        "status should report recorded port; got: {stdout}"
    );
}

#[test]
fn web_status_json_reports_recorded_running_port() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_repo(root);
    let run_dir = root.join(".kanban/run");
    fs::create_dir_all(&run_dir).expect("create run dir");
    fs::write(run_dir.join("web.pid"), format!("{}\n", std::process::id()))
        .expect("write pid file");
    fs::write(run_dir.join("web.port"), "3001\n").expect("write port file");

    let out = kanban_in(root, &["--format", "json", "web", "status", &repo_root]);

    let json = parse_stdout(&out);
    assert!(
        out.status.success(),
        "web status should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["data"]["state"], "running");
    assert!(
        json["data"]["url"]
            .as_str()
            .is_some_and(|url| url.ends_with(":3001")),
        "status JSON should report recorded port; got: {}",
        json["data"]["url"]
    );
}

#[test]
fn web_log_follow_json_emits_specific_error() {
    let dir = tempdir().expect("temp dir should be created");
    let root = dir.path();
    let repo_root = root.to_string_lossy().into_owned();

    init_repo(root);

    let out = kanban_in(
        root,
        &["--format", "json", "web", "log", "--follow", &repo_root],
    );

    let json = parse_stdout(&out);
    assert_eq!(out.status.code(), Some(1), "exit code should be 1");
    assert_eq!(json["status"], "error", "envelope status should be error");
    assert_eq!(json["kind"], "web.log", "kind should be web.log");
    assert_eq!(json["error"]["code"], "invalid_argument");
}
