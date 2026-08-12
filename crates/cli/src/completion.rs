use crate::cli::BASH_DATE_PLACEHOLDER;
use kanban_core::{CANONICAL_STORY_STATUSES, CANONICAL_TASK_STATUSES};

const ZSH_DYNAMIC_SECTIONS: &str = include_str!("completion/dynamic.zsh");
const BASH_DYNAMIC_SECTIONS: &str = include_str!("completion/dynamic.bash");

fn completion_section(sections: &'static str, name: &str) -> &'static str {
    let start_marker = format!("__KANBAN_SECTION__{name}__START__");
    let end_marker = format!("__KANBAN_SECTION__{name}__END__");
    let start = sections
        .find(&start_marker)
        .unwrap_or_else(|| panic!("missing completion section start marker: {name}"))
        + start_marker.len();
    let rest = &sections[start..];
    let end = rest
        .find(&end_marker)
        .unwrap_or_else(|| panic!("missing completion section end marker: {name}"));
    &rest[..end]
}

fn zsh_section(name: &str) -> &'static str {
    let section = completion_section(ZSH_DYNAMIC_SECTIONS, name);
    section.strip_prefix('\n').unwrap_or(section)
}

fn bash_section(name: &str) -> &'static str {
    let section = completion_section(BASH_DYNAMIC_SECTIONS, name);
    let section = section.strip_prefix('\n').unwrap_or(section);
    section.strip_suffix('\n').unwrap_or(section)
}

fn bash_helper_section(name: &str) -> &'static str {
    let section = completion_section(BASH_DYNAMIC_SECTIONS, name);
    section.strip_prefix('\n').unwrap_or(section)
}

/// Enhance the zsh completion script by replacing `_default` completions for
/// sprint name, story/epic ID, story update options, task status, and doctor
/// fix target arguments with dynamic lookup helpers.
pub(crate) fn enhance_zsh_completion(script: &str) -> String {
    let enhanced = script
        .replace(
            "':phase -- Phase identifier to inspect, for example 1 or F1.:_default'",
            "':phase -- Phase identifier to inspect, for example 1 or F1.:_kanban_phase_ids'",
        )
        // Sprint name arguments
        .replace(
            "'::name -- Sprint name to inspect, for example S001.foundation. Defaults to the current sprint.:_default'",
            "'::name -- Sprint name to inspect, for example S001.foundation. Defaults to the current sprint.:_kanban_sprint_names'",
        )
        .replace(
            "':name -- Sprint name to close and roll over.:_default'",
            "':name -- Sprint name to close and roll over.:_kanban_sprint_names'",
        )
        // Story plan sprint argument
        .replace(
            "':sprint -- Target sprint name or Snnn prefix, for example S001.planning or S001.:_default'",
            "':sprint -- Target sprint name or Snnn prefix, for example S001.planning or S001.:_kanban_sprint_names'",
        )
        .replace(
            "'--sprint=[List stories assigned to the specified sprint, for example S001.foundation.]:ID:_default'",
            "'--sprint=[List stories assigned to the specified sprint, for example S001.foundation.]:ID:_kanban_sprint_names'",
        )
        .replace(
            "'--sprint=[Target sprint name or Snnn prefix, for example S001.planning or S001.]:SPRINT:_default'",
            "'--sprint=[Target sprint name or Snnn prefix, for example S001.planning or S001.]:SPRINT:_kanban_sprint_names'",
        )
        // Story update --sprint option
        .replace(
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]:SPRINT:_default'",
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]:SPRINT:_kanban_sprint_names'",
        )
        // Story ID arguments (story show, story move, story delete, task add, task update)
        .replace(
            "':id -- Story id to inspect, for example US-F1-053.:_default'",
            "':id -- Story id to inspect, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "'--epic=[Parent epic id, for example EP-F1-06.]:EPIC:_default'",
            "'--epic=[Parent epic id, for example EP-F1-06.]:EPIC:_kanban_epic_ids'",
        )
        .replace(
            "':id -- Epic id to inspect, for example EP-F1-06.:_default'",
            "':id -- Epic id to inspect, for example EP-F1-06.:_kanban_epic_ids'",
        )
        .replace(
            "':id -- Story id to update, for example US-F1-053.:_default'",
            "':id -- Story id to update, for example US-F1-053.:_kanban_story_or_epic_ids'",
        )
        .replace(
            "':id -- Epic id to update, for example EP-F1-02.:_default'",
            "':id -- Epic id to update, for example EP-F1-02.:_kanban_epic_ids'",
        )
        .replace(
            "':id -- Story id to move, for example US-F1-053.:_default'",
            "':id -- Story id to move, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "':id -- Story id to delete, for example US-F1-053.:_default'",
            "':id -- Story id to delete, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "':id -- Backlog story id to plan, for example US-F2-001.:_default'",
            "':id -- Backlog story id to plan, for example US-F2-001.:_kanban_story_ids'",
        )
        .replace(
            "'--id=[Update frontmatter id. Omit VALUE to prompt with the current value.]::ID:_default'",
            "'--id=[Update frontmatter id. Omit VALUE to prompt with the current value.]::ID:_kanban_story_or_epic_ids'",
        )
        .replace(
            "'--type=[Update frontmatter type. Omit VALUE to prompt with the current value.]::TYPE:_default'",
            "'--type=[Update frontmatter type. Omit VALUE to prompt with the current value.]::TYPE:_kanban_story_types'",
        )
        .replace(
            "'--status=[Update frontmatter status. Omit VALUE to prompt with the current value.]::STATUS:_default'",
            "'--status=[Update frontmatter status. Omit VALUE to prompt with the current value.]::STATUS:_kanban_story_update_statuses'",
        )
        .replace(
            "'--epic=[Update frontmatter epic. Omit VALUE to prompt with the current value.]::EPIC:_default'",
            "'--epic=[Update frontmatter epic. Omit VALUE to prompt with the current value.]::EPIC:_kanban_epic_ids'",
        )
        .replace(
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]::SPRINT:_default'",
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]::SPRINT:_kanban_sprint_names'",
        )
        .replace(
            "'--story-points=[Update frontmatter story_points. Omit VALUE to prompt with the current value.]::POINTS:_default'",
            "'--story-points=[Update frontmatter story_points. Omit VALUE to prompt with the current value.]::POINTS:_kanban_story_point_values'",
        )
        .replace(
            "'--story-points=[Initial story_points value. Defaults to 5.]:POINTS:_default'",
            "'--story-points=[Initial story_points value. Defaults to 5.]:POINTS:_kanban_story_point_values'",
        )
        .replace(
            "'--planned-start=[Update frontmatter planned_start. Omit VALUE to prompt with the current value.]::DATE:_default'",
            "'--planned-start=[Update frontmatter planned_start. Omit VALUE to prompt with the current value.]::DATE:'",
        )
        .replace(
            "'--planned-end=[Update frontmatter planned_end. Omit VALUE to prompt with the current value.]::DATE:_default'",
            "'--planned-end=[Update frontmatter planned_end. Omit VALUE to prompt with the current value.]::DATE:'",
        )
        .replace(
            "'--work-started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--work-started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "'--work-done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--work-done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "':id -- Sprint story id to move, for example US-F1-053.:_default'",
            "':id -- Sprint story id to move, for example US-F1-053.:_kanban_story_ids'",
        )
        // Note: .replace replaces ALL occurrences — intentional for task add/update/delete
        .replace(
            "':story_id -- Parent story id for the task, for example US-F1-053.:_default'",
            "':story_id -- Parent story id for the task, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "':task_id -- Task id to update, for example TASK-US-F1-053-001.:_default'",
            "':task_id -- Task id to update, for example TASK-US-F1-053-001.:_kanban_task_ids_for_story'",
        )
        .replace(
            "':task_id -- Task id to delete, for example TASK-US-F1-053-001.:_default'",
            "':task_id -- Task id to delete, for example TASK-US-F1-053-001.:_kanban_task_ids_for_story'",
        )
        .replace(
            "':story_id -- Story id whose task IDs should be listed, for example US-F1-053.:_default'",
            "':story_id -- Story id whose task IDs should be listed, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "\":: :_kanban__subcmd__doctor_commands\"",
            "\":: :_kanban_doctor_command_or_repo_root\"",
        )
        .replace(
            "'::target -- Optional scope\\: a story id like US-F1-053 or the literal `current`.:_default'",
            "'::target -- Optional scope\\: a story id like US-F1-053 or the literal `current`.:_kanban_doctor_fix_targets'",
        )
        .replace(
            "':key -- Configuration key, for example paths.backlog or theme.color_mode.:_default'",
            "':key -- Configuration key, for example paths.backlog or theme.color_mode.:_kanban_config_keys'",
        )
        .replace(
            "':value -- Configuration value. Use comma-separated values for story_points.allowed_values.:_default'",
            "':value -- Configuration value. Use comma-separated values for story_points.allowed_values.:_kanban_config_values'",
        )
        // Story move status argument
        .replace(
            "':status -- Target status, for example backlog, ready, planned, todo, in-progress, ready-for-qa, done, or blocked.:_default'",
            "':status -- Target status, for example backlog, ready, planned, todo, in-progress, ready-for-qa, done, or blocked.:_kanban_story_statuses'",
        )
        .replace(
            r#"'-a+[Override assignee when moving to in-progress. Use \`Name <email>\` or a comma-separated list of assignees; invalid values fail before files are moved.]:NAME <EMAIL>:_default'"#,
            r#"'-a+[Override assignee when moving to in-progress. Use \`Name <email>\` or a comma-separated list of assignees; invalid values fail before files are moved.]:NAME <EMAIL>:'"#,
        )
        .replace(
            r#"'--assignee=[Override assignee when moving to in-progress. Use \`Name <email>\` or a comma-separated list of assignees; invalid values fail before files are moved.]:NAME <EMAIL>:_default'"#,
            r#"'--assignee=[Override assignee when moving to in-progress. Use \`Name <email>\` or a comma-separated list of assignees; invalid values fail before files are moved.]:NAME <EMAIL>:'"#,
        )
        .replace(
            "'--assignee=[Update frontmatter assignee. Use `Name <email>` or a comma-separated list. Omit VALUE to prompt with the current value.]::ASSIGNEE:_default'",
            "'--assignee=[Update frontmatter assignee. Use `Name <email>` or a comma-separated list. Omit VALUE to prompt with the current value.]::ASSIGNEE:'",
        )
        // Task add/update status argument and option
        .replace(
            "'--status=[Initial task status to write. Defaults to todo.]:STATUS:_default'",
            "'--status=[Initial task status to write. Defaults to todo.]:STATUS:_kanban_task_statuses'",
        )
        .replace(
            "'--status=[Replacement task status. Omitted means keep the current status.]:STATUS:_default'",
            "'--status=[Replacement task status. Omitted means keep the current status.]:STATUS:_kanban_task_statuses'",
        )
        .replace(
            "'--title=[Task title to append to the sibling task log.]:TITLE:_default'",
            "'--title=[Task title to append to the sibling task log.]:TITLE:'",
        )
        .replace(
            "'*--tags=[Comma-separated task tags to write.]:TAGS:_default'",
            "'*--tags=[Comma-separated task tags to write.]:TAGS:'",
        )
        .replace(
            "'--description=[Task description to write in the task log.]:DESCRIPTION:_default'",
            "'--description=[Task description to write in the task log.]:DESCRIPTION:'",
        )
        .replace(
            "'--title=[Replacement task title. Omitted means keep the current title.]:TITLE:_default'",
            "'--title=[Replacement task title. Omitted means keep the current title.]:TITLE:'",
        )
        .replace(
            "'*--tags=[Replacement comma-separated task tags. Omitted means keep current tags.]:TAGS:_default'",
            "'*--tags=[Replacement comma-separated task tags. Omitted means keep current tags.]:TAGS:'",
        )
        .replace(
            "'--description=[Replacement task description. Omitted means keep the current description.]:DESCRIPTION:_default'",
            "'--description=[Replacement task description. Omitted means keep the current description.]:DESCRIPTION:'",
        )
        // Sprint create date options
        .replace(
            "'--number=[Sprint number. Defaults to the next suggested number.]:N:_default'",
            "'--number=[Sprint number. Defaults to the next suggested number.]:N:'",
        )
        .replace(
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:_default'",
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:'",
        )
        .replace(
            "'--start=[Start date. Defaults to the suggested next start date.]:YYYY-MM-DD:_default'",
            "'--start=[Start date. Defaults to the suggested next start date.]:YYYY-MM-DD:'",
        )
        .replace(
            "'--end=[End date. Defaults to the suggested next end date.]:YYYY-MM-DD:_default'",
            "'--end=[End date. Defaults to the suggested next end date.]:YYYY-MM-DD:'",
        )
        // Story update date options
        .replace(
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "'--work_started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--work_started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--work-started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--work-started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "'--work_done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--work_done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--work-done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--work-done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        .replace(
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]::TIMESTAMP:_default'",
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]::TIMESTAMP:'",
        )
        // Web log lines option
        .replace(
            "'--lines=[Only print the last N log lines.]:N:_default'",
            "'--lines=[Only print the last N log lines.]:N:'",
        );
    let story_status_lines = CANONICAL_STORY_STATUSES
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_string()
            } else {
                format!("        {s}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let task_status_lines = CANONICAL_TASK_STATUSES
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_string()
            } else {
                format!("        {s}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let zsh_helpers = zsh_section("ZSH_DYNAMIC_HELPERS")
        .replace("__KANBAN_STORY_STATUSES__", &story_status_lines)
        .replace("__KANBAN_TASK_STATUSES__", &task_status_lines);
    format!(
        "{enhanced}{zsh_helpers}{}",
        zsh_section("ZSH_KB_ALIAS_REGISTRATION")
    )
}

/// Inject dynamic completion into a single bash case block identified by its label and opts string.
/// Inserts a story/sprint lookup BEFORE the standard opts fallback at the given word position.
pub(crate) fn inject_bash_dynamic(
    script: &str,
    label: &str,
    opts: &str,
    kind: &str,
    pos: usize,
) -> String {
    let old = format!(
        "        {label})\n            opts=\"{opts}\"\n            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {pos} ]] ; then\n                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n                return 0\n            fi"
    );
    let new = format!(
        "        {label})\n            opts=\"{opts}\"\n            if [[ ${{COMP_CWORD}} -eq {pos} && ${{cur}} != -* ]]; then\n                local -a matches=()\n                local id\n                while IFS= read -r id; do\n                    [[ -n \"$id\" && \"$id\" == *\"${{cur}}\"* ]] && matches+=( \"$id\" )\n                done < <(kanban list-ids {kind} 2>/dev/null)\n                COMPREPLY=( \"${{matches[@]}}\" )\n                return 0\n            fi\n            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {pos} ]] ; then\n                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n                return 0\n            fi"
    );
    if script.contains(&old) {
        script.replacen(&old, &new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn replace_bash_case_block(script: &str, label: &str, replacement: &str) -> String {
    let start_marker = format!("        {label})\n");
    let Some(start) = script.find(&start_marker) else {
        return script.to_string();
    };
    let search_start = start + start_marker.len();
    let Some(next) = script[search_start..]
        .find("\n        kanban__")
        .map(|offset| search_start + offset + 1)
    else {
        return script.to_string();
    };

    let mut result =
        String::with_capacity(script.len() + replacement.len().saturating_sub(next - start));
    result.push_str(&script[..start]);
    result.push_str(replacement);
    result.push_str(&script[next..]);
    result
}

pub(crate) fn inject_bash_phase_show(script: &str) -> String {
    let replacement = bash_section("INJECT_BASH_PHASE_SHOW_REPLACEMENT");
    replace_bash_case_block(script, "kanban__subcmd__phase__subcmd__show", replacement)
}

pub(crate) fn inject_bash_story_list(script: &str) -> String {
    let replacement = bash_section("INJECT_BASH_STORY_LIST_REPLACEMENT");
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__list", replacement)
}

pub(crate) fn inject_bash_list_task_ids(script: &str) -> String {
    let replacement = bash_section("INJECT_BASH_LIST_TASK_IDS_REPLACEMENT");
    replace_bash_case_block(
        script,
        "kanban__subcmd__list__subcmd__task__subcmd__ids",
        replacement,
    )
}

pub(crate) fn inject_bash_doctor_fix_target(script: &str) -> String {
    let old = bash_section("INJECT_BASH_DOCTOR_FIX_TARGET_OLD");
    let new = bash_section("INJECT_BASH_DOCTOR_FIX_TARGET_NEW");
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_doctor_command_or_repo_root(script: &str) -> String {
    let old = bash_section("INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_OLD");
    let new = bash_section("INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_NEW");
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_config_get(script: &str) -> String {
    let old = bash_section("INJECT_BASH_CONFIG_GET_OLD");
    let new = bash_section("INJECT_BASH_CONFIG_GET_NEW");
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_config_set(script: &str) -> String {
    let old = bash_section("INJECT_BASH_CONFIG_SET_OLD");
    let new = bash_section("INJECT_BASH_CONFIG_SET_NEW");
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_sprint_create(script: &str) -> String {
    let old = bash_section("INJECT_BASH_SPRINT_CREATE_OLD");
    let new = bash_section("INJECT_BASH_SPRINT_CREATE_NEW")
        .replace("__KANBAN_DATE_PLACEHOLDER__", BASH_DATE_PLACEHOLDER);
    if script.contains(old) {
        script.replacen(old, &new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_web_log(script: &str) -> String {
    let old = bash_section("INJECT_BASH_WEB_LOG_OLD");
    let new = bash_section("INJECT_BASH_WEB_LOG_NEW");
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_story_plan(script: &str) -> String {
    let replacement = bash_section("INJECT_BASH_STORY_PLAN_REPLACEMENT");
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__plan", replacement)
}

pub(crate) fn inject_bash_story_move_status(script: &str) -> String {
    let story_statuses = CANONICAL_STORY_STATUSES.join(" ");
    let replacement = bash_section("INJECT_BASH_STORY_MOVE_STATUS_REPLACEMENT");
    let replacement = replacement.replace("__KANBAN_STORY_STATUSES__", &story_statuses);
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__move", &replacement)
}

pub(crate) fn inject_bash_task_add_status(script: &str) -> String {
    let task_statuses = CANONICAL_TASK_STATUSES.join(" ");
    let replacement = bash_section("INJECT_BASH_TASK_ADD_STATUS_REPLACEMENT");
    let replacement = replacement.replace("__KANBAN_TASK_STATUSES__", &task_statuses);
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__add", &replacement)
}

pub(crate) fn inject_bash_task_update_status(script: &str) -> String {
    let task_statuses = CANONICAL_TASK_STATUSES.join(" ");
    let replacement = bash_section("INJECT_BASH_TASK_UPDATE_STATUS_REPLACEMENT");
    let replacement = replacement.replace("__KANBAN_TASK_STATUSES__", &task_statuses);
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__update", &replacement)
}

pub(crate) fn inject_bash_task_delete(script: &str) -> String {
    let replacement = bash_section("INJECT_BASH_TASK_DELETE_REPLACEMENT");
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__delete", replacement)
}

/// Enhance the bash completion script with dynamic sprint name, story/epic ID,
/// and doctor fix target completions.
pub(crate) fn enhance_bash_completion(script: &str) -> String {
    let script = inject_bash_doctor_command_or_repo_root(script);
    let script = inject_bash_sprint_create(&script);
    let script = inject_bash_phase_show(&script);
    let script = inject_bash_story_list(&script);
    let script = inject_bash_list_task_ids(&script);
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__epic__subcmd__show",
        "-h --format --help <ID> [REPO_ROOT]",
        "epics",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__epic__subcmd__update",
        "-h --priority --planned-start --planned-end --work-started --work-done --format --help <ID> [REPO_ROOT]",
        "epics",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__show",
        "-h --format --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__rollover",
        "-h --format --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__story__subcmd__show",
        "-h --format --help <ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_story_update_dynamic(&script);
    let script = inject_bash_story_move_status(&script);
    let script = inject_bash_story_plan(&script);
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__add",
        "-h --title --status --tags --description --format --help <STORY_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__update",
        "-h --title --status --tags --description --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_task_add_status(&script);
    let script = inject_bash_task_update_status(&script);
    let script = inject_bash_task_delete(&script);
    let script = inject_bash_doctor_fix_target(&script);
    let script = inject_bash_config_get(&script);
    let script = inject_bash_config_set(&script);
    let script = inject_bash_web_log(&script);
    let script = make_bash_id_matches_case_insensitive(&script);
    let script = append_bash_ci_helper(&script);
    append_bash_kb_alias(&script)
}

/// Rewrite the prefix/substring ID match used by the injected dynamic loops so
/// it matches case-insensitively. The matched idiom is identical across every
/// injected story/epic/task lookup, so a single replacement covers them all.
pub(crate) fn make_bash_id_matches_case_insensitive(script: &str) -> String {
    script.replace(
        r#"[[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )"#,
        r#"{ [[ -n "$id" ]] && _kanban_ci_match "$id" "${cur}"; } && matches+=( "$id" )"#,
    )
}

/// Append the shared case-insensitive match helper to the bash script.
pub(crate) fn append_bash_ci_helper(script: &str) -> String {
    format!(
        "{script}{}{}",
        bash_helper_section("BASH_CI_MATCH_HELPER"),
        bash_helper_section("BASH_RESOLVE_STORY_ID_HELPER")
    )
}

/// Register the documented `kb` alias for the same completion function as
/// `kanban`, mirroring clap_complete's bash-version-aware `complete` call.
pub(crate) fn append_bash_kb_alias(script: &str) -> String {
    let registration = bash_helper_section("APPEND_BASH_KB_ALIAS_REGISTRATION");
    format!("{script}{registration}")
}

#[allow(dead_code)]
pub(crate) fn inject_bash_story_update(_script: &str) -> String {
    String::new()
}

pub(crate) fn inject_bash_story_update_dynamic(script: &str) -> String {
    let start_marker = "        kanban__subcmd__story__subcmd__update)\n";
    let end_marker = "        kanban__subcmd__task)\n";
    let Some(start) = script.find(start_marker) else {
        return script.to_string();
    };
    let Some(end) = script[start..]
        .find(end_marker)
        .map(|offset| start + offset)
    else {
        return script.to_string();
    };

    let replacement = bash_section("INJECT_BASH_STORY_UPDATE_DYNAMIC_REPLACEMENT");

    let mut result =
        String::with_capacity(script.len() + replacement.len().saturating_sub(end - start));
    result.push_str(&script[..start]);
    result.push_str(replacement);
    result.push_str(&script[end..]);
    result
}
