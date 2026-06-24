use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use kanban_core::*;

use crate::dto::WebTeamMember;

pub(crate) fn load_team(repo_root: &Path) -> Result<Vec<WebTeamMember>> {
    let config = load_kanban_config(repo_root)?;
    if !config.team.is_empty() {
        return Ok(map_team_members(config.team));
    }

    let team_file = repo_root.join(".kanban/team.json");
    if let Ok(raw) = fs::read_to_string(&team_file)
        && let Ok(team) = serde_json::from_str::<Vec<TeamMemberConfig>>(&raw)
    {
        return Ok(map_team_members(team));
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

pub(crate) fn map_team_members(team: Vec<TeamMemberConfig>) -> Vec<WebTeamMember> {
    let avatars_prefix = "/api/team/avatars/";
    team.into_iter()
        .filter_map(|member| {
            let TeamMemberConfig {
                name,
                email,
                avatar_url,
                avatar_path,
            } = member;
            let name = name.trim().to_string();
            let email = email.trim().to_string();
            if name.is_empty() || email.is_empty() {
                return None;
            }
            let avatar_url = avatar_url
                .and_then(|value| {
                    let trimmed = value.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .or_else(|| {
                    avatar_path.and_then(|path| {
                        let trimmed = path.trim();
                        (!trimmed.is_empty())
                            .then(|| format!("{avatars_prefix}{}", trimmed.trim_start_matches('/')))
                    })
                });
            Some(WebTeamMember {
                label: format!("{name} <{email}>"),
                avatar_url,
                name,
                email,
            })
        })
        .collect()
}

pub(crate) fn parse_assignees(raw: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use tempfile::tempdir;

    #[test]
    fn load_team_prefers_settings_team_members() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();
        let settings_path = temp_root.path().join(".kanban/settings.json");
        let mut settings =
            serde_json::from_str::<Value>(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        settings["team"] = json!([
            {"name": "Configured User", "email": "configured@example.com", "avatarPath": "configured.png"}
        ]);
        fs::write(
            &settings_path,
            format!("{}\n", serde_json::to_string_pretty(&settings).unwrap()),
        )
        .unwrap();
        fs::write(
            temp_root.path().join(".kanban/team.json"),
            "[{\"name\":\"Legacy User\",\"email\":\"legacy@example.com\"}]\n",
        )
        .unwrap();
        let team = load_team(temp_root.path()).unwrap();
        assert_eq!(team.len(), 1);
        assert_eq!(team[0].name, "Configured User");
        assert_eq!(team[0].email, "configured@example.com");
        assert_eq!(
            team[0].avatar_url.as_deref(),
            Some("/api/team/avatars/configured.png")
        );
    }

    #[test]
    fn load_team_falls_back_to_legacy_team_file() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();
        let settings_path = temp_root.path().join(".kanban/settings.json");
        let mut settings =
            serde_json::from_str::<Value>(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        settings["team"] = json!([]);
        fs::write(
            &settings_path,
            format!("{}\n", serde_json::to_string_pretty(&settings).unwrap()),
        )
        .unwrap();
        fs::write(temp_root.path().join(".kanban/team.json"), "[{\"name\":\"Legacy User\",\"email\":\"legacy@example.com\",\"avatarUrl\":\"https://example.com/avatar.png\"}]\n").unwrap();
        let team = load_team(temp_root.path()).unwrap();
        assert_eq!(team.len(), 1);
        assert_eq!(team[0].name, "Legacy User");
        assert_eq!(team[0].email, "legacy@example.com");
        assert_eq!(
            team[0].avatar_url.as_deref(),
            Some("https://example.com/avatar.png")
        );
    }
}
