//! Deterministic backlog fixture generation for benchmarks and equivalence
//! tests.
//!
//! Compiled only for `cargo test` or with the `test-support` feature so it
//! never ships in a release binary.
//!
//! The generator exists because the profiling that motivated the read-path
//! rework was done against an external checkout. Every assertion in the test
//! suite must instead be reproducible from a fixture that is created here, in a
//! `tempdir`, and `git init`-ed so `resolve_repo_root` exercises the real
//! subprocess path.
//!
//! Feature flags are pinned explicitly (never inherited from `kanban init`
//! defaults or from the host repository) because a fixture with
//! `features.sprints = false` skips the sprint derivation entirely and would
//! silently under-measure the read path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{FeaturesConfig, init_config_with_features, set_config_value};

/// Shape of a generated backlog fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureSpec {
    pub stories: usize,
    pub epics: usize,
    pub sprints: usize,
    /// How many stories get a sibling `<story>.tasks.md` file.
    pub sibling_task_files: usize,
    pub features: FeaturesConfig,
}

impl FixtureSpec {
    /// The representative fixture: 250 stories, 30 epics, 5 sprints, ~180
    /// sibling task files, sprints and epics enabled.
    pub fn representative() -> Self {
        Self {
            stories: 250,
            epics: 30,
            sprints: 5,
            sibling_task_files: 180,
            features: FeaturesConfig {
                phases: false,
                sprints: true,
                epics: true,
            },
        }
    }

    /// The minimal fixture mirroring a repository that runs with sprints
    /// disabled, so both configurations stay covered.
    pub fn minimal() -> Self {
        Self {
            stories: 40,
            epics: 6,
            sprints: 0,
            sibling_task_files: 20,
            features: FeaturesConfig {
                phases: false,
                sprints: false,
                epics: true,
            },
        }
    }

    pub fn with_stories(mut self, stories: usize) -> Self {
        self.stories = stories;
        self
    }
}

/// A generated fixture repository. Keep the value alive: dropping it removes
/// the temporary directory.
#[derive(Debug)]
pub struct BacklogFixture {
    dir: tempfile::TempDir,
    root: PathBuf,
    spec: FixtureSpec,
}

impl BacklogFixture {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn spec(&self) -> &FixtureSpec {
        &self.spec
    }

    /// Leak the temp directory so an external process (a benchmark harness, a
    /// manually started `kanban web serve`) can use it. The caller owns cleanup.
    pub fn keep(self) -> PathBuf {
        let path = self.root.clone();
        let _ = self.dir.keep();
        path
    }
}

/// Statuses used for the generated distribution. Covers every board bucket plus
/// `dropped` (excluded from scope) and the `In Progress` alias.
const STATUS_CYCLE: [&str; 12] = [
    "todo",
    "in-progress",
    "done",
    "done",
    "ready-for-qa",
    "blocked",
    "planned",
    "done",
    "In Progress",
    "todo",
    "dropped",
    "done",
];

const POINTS_CYCLE: [&str; 6] = ["1", "2", "3", "5", "8", "13"];

pub fn generate_backlog_fixture(spec: &FixtureSpec) -> BacklogFixture {
    let dir = tempfile::tempdir().expect("create fixture tempdir");
    let root = dir.path().to_path_buf();

    git_init(&root);
    init_config_with_features(&root, Some(spec.features)).expect("init fixture config");
    if !spec.features.sprints {
        set_config_value(&root, "paths.sprints", "").expect("clear sprints path");
    }
    // `init_config_with_features` resolves the repo root through git, so the
    // canonicalized root is what every later path must be built from.
    let root = crate::config::resolve_repo_root(&root).expect("resolve fixture root");

    let backlog = root.join("delivery/backlog");
    let sprint_names = generate_sprints(&root, spec);
    let epic_ids = generate_epics(&backlog, spec);
    generate_stories(&backlog, spec, &epic_ids, &sprint_names);

    BacklogFixture {
        dir,
        root,
        spec: spec.clone(),
    }
}

fn git_init(root: &Path) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("init")
        .arg("--quiet")
        .status()
        .expect("run git init for fixture");
    assert!(status.success(), "git init failed for fixture");
}

fn group_dir(backlog: &Path, index: usize, groups: usize) -> PathBuf {
    backlog.join(format!("group-{:02}", index % groups.max(1)))
}

fn generate_sprints(root: &Path, spec: &FixtureSpec) -> Vec<String> {
    if !spec.features.sprints || spec.sprints == 0 {
        return Vec::new();
    }
    let sprints_dir = root.join("delivery/sprints");
    fs::create_dir_all(&sprints_dir).expect("create fixture sprints dir");

    let mut names = Vec::new();
    for index in 0..spec.sprints {
        let sprint_id = format!("S{:03}", index + 1);
        let headline = format!("iteration-{}", index + 1);
        let name = format!("{sprint_id}.{headline}");
        // Dates are fixed, not derived from `Local::now`, so generated fixtures
        // are byte-identical across runs. The last sprint is the active one.
        let start = format!("2026-01-{:02}", 1 + index * 5);
        let end = format!("2026-01-{:02}", 4 + index * 5);
        let status = if index + 1 == spec.sprints {
            "active"
        } else {
            "closed"
        };
        let contents = format!(
            "---\nsprint: {sprint_id}\nheadline: {headline}\nstart_date: {start}\nend_date: {end}\nstatus: {status}\nwip_limit: 5\n---\n\n# {sprint_id}: {headline}\n\n## Sprint Goal\n\nDeliver iteration {}.\n\n## User Stories selected for sprint\n\n_No stories selected._\n",
            index + 1
        );
        fs::write(sprints_dir.join(format!("{name}.md")), contents).expect("write fixture sprint");
        names.push(name);
    }
    names
}

fn generate_epics(backlog: &Path, spec: &FixtureSpec) -> Vec<String> {
    let mut ids = Vec::new();
    for index in 0..spec.epics {
        let id = format!("EP-{:03}", index + 1);
        let dir = group_dir(backlog, index, spec.epics.max(1));
        fs::create_dir_all(&dir).expect("create fixture epic dir");
        let contents = format!(
            "---\nid: {id}\ntype: epic\nstatus: draft\nphase: F1\npriority: {}\nowner: Fixture Owner\nmilestone: MP1\nplanned_start: 2026-01-01\nplanned_end: 2026-03-31\nwork_started:\nwork_done:\ncreated: 2026-01-01T09:00:00+0100\nupdated: 2026-01-02T09:00:00+0100\n---\n\n# Epic: Fixture epic {id}\n\n---\n\n## Business Context\n\nContext for {id}.\n\n---\n\n## Acceptance Criteria\n\n- [ ] Fixture epic {id} is measurable\n\n---\n",
            (index + 1) * 10
        );
        fs::write(dir.join(format!("{id}-fixture-epic.md")), contents).expect("write fixture epic");
        ids.push(id);
    }
    ids
}

fn generate_stories(
    backlog: &Path,
    spec: &FixtureSpec,
    epic_ids: &[String],
    sprint_names: &[String],
) {
    let groups = spec.epics.max(1);
    for index in 0..spec.stories {
        let number = index + 1;
        let id = format!("US-{number:03}");
        let dir = group_dir(backlog, index, groups);
        fs::create_dir_all(&dir).expect("create fixture story dir");
        let status = STATUS_CYCLE[index % STATUS_CYCLE.len()];
        let points = POINTS_CYCLE[index % POINTS_CYCLE.len()];

        // Coverage requirements from the plan:
        //   index 0 -> no epic
        //   index 1 -> epic id with no epic file
        //   index 2 -> `task_file` frontmatter pointing at a referenced file
        // everything else -> a real epic, round-robin.
        let epic = match index {
            0 => None,
            1 => Some("EP-999".to_string()),
            _ if epic_ids.is_empty() => None,
            _ => Some(epic_ids[index % epic_ids.len()].clone()),
        };
        let sprint = if sprint_names.is_empty() {
            None
        } else {
            // Leave a slice of stories unplanned so backlog-only paths stay
            // represented.
            (index % 7 != 0).then(|| sprint_names[index % sprint_names.len()].clone())
        };

        let referenced_task_file = index == 2;
        let story_stem = format!("{id}-fixture-story");
        let story_path = dir.join(format!("{story_stem}.md"));

        let mut frontmatter = String::new();
        frontmatter.push_str(&format!("id: {id}\n"));
        frontmatter.push_str("type: user-story\n");
        frontmatter.push_str(&format!("status: {status}\n"));
        frontmatter.push_str(&format!("epic: {}\n", epic.clone().unwrap_or_default()));
        frontmatter.push_str(&format!("sprint: {}\n", sprint.clone().unwrap_or_default()));
        frontmatter.push_str(&format!(
            "assignee: Fixture Dev {} <dev{}@example.com>\n",
            index % 5,
            index % 5
        ));
        frontmatter.push_str(&format!("story_points: {points}\n"));
        frontmatter.push_str(&format!("priority: {}\n", (index % 20) + 1));
        if referenced_task_file {
            frontmatter.push_str(&format!("task_file: {story_stem}.referenced-tasks.md\n"));
        }
        if matches!(
            status,
            "in-progress" | "In Progress" | "ready-for-qa" | "done"
        ) {
            frontmatter.push_str("work_started: 2026-01-05T09:00:00+0100\n");
        } else {
            frontmatter.push_str("work_started:\n");
        }
        if status == "done" {
            frontmatter.push_str(&format!(
                "work_done: 2026-01-{:02}T17:00:00+0100\n",
                (index % 27) + 1
            ));
        } else {
            frontmatter.push_str("work_done:\n");
        }
        frontmatter.push_str("created: 2026-01-01T09:00:00+0100\n");
        frontmatter.push_str("updated: 2026-01-06T09:00:00+0100\n");

        let body = format!(
            "# User Story: Fixture story {id}\n\n---\n\n## Description\n\nAs a fixture consumer I want story {id} so that the read path is measurable.\n\n---\n\n## Acceptance Criteria\n\n### Scenario: {id} behaves deterministically\n\n```gherkin\nGiven a generated backlog fixture\nWhen the repository is read\nThen story {id} parses exactly once\n```\n\n---\n"
        );
        fs::write(&story_path, format!("---\n{frontmatter}---\n\n{body}"))
            .expect("write fixture story");

        if referenced_task_file {
            fs::write(
                dir.join(format!("{story_stem}.referenced-tasks.md")),
                task_markdown(&id),
            )
            .expect("write referenced task file");
        } else if index < spec.sibling_task_files {
            fs::write(
                dir.join(format!("{story_stem}.tasks.md")),
                task_markdown(&id),
            )
            .expect("write sibling task file");
        }
    }
}

fn task_markdown(story_id: &str) -> String {
    let statuses = ["todo", "in-progress", "done", "blocked"];
    let mut out = format!("# Tasks for {story_id}\n\nParent User Story: {story_id}\n");
    for (index, status) in statuses.iter().enumerate() {
        out.push_str(&format!(
            "\n## TASK-{:03} - Fixture task {} for {story_id}\n\nStatus: {status}\nTags: fixture, generated\n\nDescription:\nDescription for task {} of {story_id}.\n",
            index + 1,
            index + 1,
            index + 1
        ));
    }
    out
}
