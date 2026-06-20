use crate::config::*;
use crate::prelude::*;

pub(crate) fn repo_root() -> PathBuf {
    if let Some(repo_root) = std::env::var_os("KANBAN_TEST_REPO_ROOT") {
        return PathBuf::from(repo_root).canonicalize().unwrap();
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ip-2.0")
        .canonicalize()
        .unwrap()
}

pub(crate) fn init_temp_repo(temp_root: &Path) {
    init_config(temp_root).unwrap();
    fs::create_dir_all(temp_root.join("delivery/backlog")).unwrap();
    fs::create_dir_all(temp_root.join("delivery/sprints")).unwrap();
}

pub(crate) fn write_git_config(repo_root: &Path, name: &str, email: &str) {
    let init_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("init")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(init_status.success());
    let name_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg("user.name")
        .arg(name)
        .status()
        .unwrap();
    assert!(name_status.success());
    let email_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg("user.email")
        .arg(email)
        .status()
        .unwrap();
    assert!(email_status.success());
}

pub(crate) fn sprint_readme(
    sprint: &str,
    headline: &str,
    start: &str,
    end: &str,
    status: &str,
) -> String {
    format!(
        "---\nsprint: {sprint}\nheadline: {headline}\nstart_date: {start}\nend_date: {end}\nstatus: {status}\nwip_limit: null\n---\n\n# {sprint}: {headline}\n\n## Sprint Goal\n\nKeep the team aligned on a visible sprint outcome.\n"
    )
}

pub(crate) fn write_story(temp_root: &Path, relative_path: &str, frontmatter: &str) -> PathBuf {
    let relative_path = relative_path
        .strip_prefix("doc/backlog/")
        .map(|path| format!("delivery/backlog/{path}"))
        .unwrap_or_else(|| relative_path.to_string());
    let path = temp_root.join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        format!("---\n{frontmatter}---\n\n# User Story: Test story\n\n## Acceptance Criteria\n\nScenario 1\n"),
    )
    .unwrap();
    path
}

pub(crate) fn write_story_with_task_file(
    temp_root: &Path,
    relative_path: &str,
    frontmatter: &str,
) -> PathBuf {
    let path = write_story(temp_root, relative_path, frontmatter);
    fs::write(
        path.with_extension("tasks.md"),
        "# Tasks for US-F1-001\n\nParent User Story: US-F1-001\nSprint: S001.foundation\n\n## TASK-US-F1-001-001 - First task\n\nStatus: todo\nTags: cli\n\nDescription:\nInitial work.\n",
    )
    .unwrap();
    path
}

pub(crate) fn write_sprint_file(
    temp_root: &Path,
    sprint_name: &str,
    headline: &str,
    start: &str,
    end: &str,
    status: &str,
) -> PathBuf {
    let path = temp_root.join(format!("delivery/sprints/{sprint_name}.md"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let sprint_id = sprint_name.split('.').next().unwrap();
    fs::write(
        &path,
        sprint_readme(sprint_id, headline, start, end, status),
    )
    .unwrap();
    path
}
