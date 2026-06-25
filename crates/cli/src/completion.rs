#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

/// ZSH helper functions appended after the clap_complete-generated script.
/// These provide dynamic completion for config keys/values, sprint names, story IDs,
/// doctor fix targets, epic IDs, task statuses, story update option values, and phase IDs.
pub(crate) const ZSH_DYNAMIC_HELPERS: &str = r#"
_kanban_config_keys() {
    local -a keys
    keys=(
        paths.backlog
        paths.sprints
        features.sprints
        features.epics
        features.phases
        theme.color_mode
        story_points.allowed_values
        story_points.aliases.XS
        story_points.aliases.S
        story_points.aliases.M
        story_points.aliases.L
        story_points.aliases.XL
    )
    compadd -a keys
}
_kanban_config_values() {
    local key="$words[3]"
    case "$key" in
        theme.color_mode)
            compadd auto always never
            ;;
        features.sprints|features.epics|features.phases)
            compadd true false on off yes no 1 0
            ;;
        paths.backlog|paths.sprints)
            _files -/
            ;;
        *)
            _default
            ;;
    esac
}
_kanban_sprint_names() {
    local -a names
    local name
    while IFS= read -r name; do
        [[ -n "$name" ]] && names+=( "$name" )
    done < <(kanban list-ids sprints 2>/dev/null)
    compadd -a names
}
_kanban_story_ids() {
    local -a ids descriptions
    local id title
    local needle="$PREFIX"
    while IFS=$'\t' read -r id title; do
        [[ -z "$id" ]] && continue
        if [[ -z "$needle" || "${(L)id}" == *"${(L)needle}"* ]]; then
            ids+=( "$id" )
            if [[ -n "$title" ]]; then
                descriptions+=( "$id -- $title" )
            else
                descriptions+=( "$id" )
            fi
        fi
    done < <(kanban list-ids stories-with-titles 2>/dev/null)
    compadd -U -d descriptions -a ids
}
_kanban_story_or_epic_ids() {
    local -a ids
    local id needle="$PREFIX"
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "${(L)id}" == *"${(L)needle}"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids stories 2>/dev/null)
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "${(L)id}" == *"${(L)needle}"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids epics 2>/dev/null)
    compadd -U -a ids
}
_kanban_story_types() {
    compadd user-story epic
}
_kanban_story_update_statuses() {
    local -a statuses
    statuses=(
        __KANBAN_STORY_STATUSES__
    )
    compadd -a statuses
}
_kanban_story_point_values() {
    local -a values
    local value
    while IFS= read -r value; do
        [[ -n "$value" ]] && values+=( "$value" )
    done < <(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[]",' | tr '[:space:]' '\n')
    compadd -a values
}
_kanban_resolve_story_id() {
    local candidate="$1"
    local id
    [[ -z "$candidate" ]] && return 1
    while IFS= read -r id; do
        if [[ "$id" == "$candidate" ]]; then
            print -r -- "$id"
            return 0
        fi
    done < <(kanban list-ids stories 2>/dev/null)
    return 1
}
_kanban_phase_ids() {
    compadd F1 F2 F3 F4 F5 1 2 3 4 5
}
_kanban_task_ids_for_story() {
    local -a ids
    local id story_id
    story_id=$(_kanban_resolve_story_id "${words[CURRENT-1]}")
    [[ -z "$story_id" ]] && return 0
    while IFS= read -r id; do
        [[ -n "$id" ]] && ids+=( "$id" )
    done < <(kanban list-task-ids "$story_id" 2>/dev/null)
    compadd -a ids
}
_kanban_doctor_fix_targets() {
    local -a ids descriptions
    local id title
    ids=( current )
    descriptions=( "current -- current active sprint" )
    while IFS=$'\t' read -r id title; do
        [[ -z "$id" ]] && continue
        ids+=( "$id" )
        if [[ -n "$title" ]]; then
            descriptions+=( "$id -- $title" )
        else
            descriptions+=( "$id" )
        fi
    done < <(kanban list-ids stories-with-titles 2>/dev/null)
    compadd -U -d descriptions -a ids
}
_kanban_doctor_command_or_repo_root() {
    _alternative \
        'command:doctor command:(show fix help)' \
        'repo-root:repository root:_files -/'
}
_kanban_epic_ids() {
    local -a ids
    local id
    local needle="$PREFIX"
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "${(L)id}" == *"${(L)needle}"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids epics 2>/dev/null)
    compadd -U -a ids
}
_kanban_task_statuses() {
    local -a statuses
    statuses=(
        __KANBAN_TASK_STATUSES__
    )
    compadd -a statuses
}
_kanban_story_statuses() {
    local -a statuses
    statuses=(
        __KANBAN_STORY_STATUSES__
    )
    compadd -a statuses
}
"#;

/// Enhance the zsh completion script by replacing `_default` completions for
/// sprint name, story ID, story update options, task status, and doctor fix target arguments with dynamic lookup helpers.
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
            "':id -- Story id to update, for example US-F1-053.:_default'",
            "':id -- Story id to update, for example US-F1-053.:_kanban_story_or_epic_ids'",
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
            "':status -- Target status, for example backlog, ready, todo, in-progress, ready-for-qa, done, or blocked.:_default'",
            "':status -- Target status, for example backlog, ready, todo, in-progress, ready-for-qa, done, or blocked.:_kanban_story_statuses'",
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
    let zsh_helpers = ZSH_DYNAMIC_HELPERS
        .replace("__KANBAN_STORY_STATUSES__", &story_status_lines)
        .replace("__KANBAN_TASK_STATUSES__", &task_status_lines);
    format!("{enhanced}{zsh_helpers}{ZSH_KB_ALIAS_REGISTRATION}")
}

/// Register the documented `kb` alias for the same completion function as `kanban`.
/// Appended after the clap_complete-generated `compdef _kanban kanban` registration.
pub(crate) const ZSH_KB_ALIAS_REGISTRATION: &str = r#"
if [ "$funcstack[1]" != "_kanban" ]; then
    compdef _kanban kb
fi
"#;

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
    let replacement = r#"        kanban__subcmd__phase__subcmd__show)
            opts="-h --format --help <PHASE> [REPO_ROOT]"
            phases="F1 F2 F3 F4 F5 1 2 3 4 5"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${phases}" -- "${cur}") )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
"#;
    replace_bash_case_block(script, "kanban__subcmd__phase__subcmd__show", replacement)
}

pub(crate) fn inject_bash_story_list(script: &str) -> String {
    let replacement = r#"        kanban__subcmd__story__subcmd__list)
            opts="-h --current --all --next --sprint --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --sprint)
                    COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
"#;
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__list", replacement)
}

pub(crate) fn inject_bash_list_task_ids(script: &str) -> String {
    let replacement = r#"        kanban__subcmd__list__subcmd__task__subcmd__ids)
            opts="-h --format --help <STORY_ID> [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 2 && ${cur} != -* ]] ; then
                local -a matches=()
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
"#;
    replace_bash_case_block(
        script,
        "kanban__subcmd__list__subcmd__task__subcmd__ids",
        replacement,
    )
}

pub(crate) fn inject_bash_doctor_fix_target(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --non-interactive --format --help [TARGET] [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --non-interactive --format --help [TARGET] [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=( current )
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_doctor_command_or_repo_root(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor)
            opts="-h --format --help show fix help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__doctor)
            opts="-h --format --help show fix help"
            doctor_commands="show fix help"
            if [[ ${COMP_CWORD} -eq 2 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${doctor_commands}" -- "${cur}") $(compgen -d -- "${cur}") )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_config_get(script: &str) -> String {
    let old = r#"        kanban__subcmd__config__subcmd__get)
            opts="-h --format --help <KEY> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__config__subcmd__get)
            opts="-h --format --help <KEY> [REPO_ROOT]"
            config_keys="paths.backlog paths.sprints features.sprints features.epics features.phases theme.color_mode story_points.allowed_values story_points.aliases.XS story_points.aliases.S story_points.aliases.M story_points.aliases.L story_points.aliases.XL"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${config_keys}" -- "${cur}") )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_config_set(script: &str) -> String {
    let old = r#"        kanban__subcmd__config__subcmd__set)
            opts="-h --format --help <KEY> <VALUE> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__config__subcmd__set)
            opts="-h --format --help <KEY> <VALUE> [REPO_ROOT]"
            config_keys="paths.backlog paths.sprints features.sprints features.epics features.phases theme.color_mode story_points.allowed_values story_points.aliases.XS story_points.aliases.S story_points.aliases.M story_points.aliases.L story_points.aliases.XL"
            color_modes="auto always never"
            feature_flags="true false on off yes no 1 0"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${config_keys}" -- "${cur}") )
                return 0
            fi
            if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                case "${prev}" in
                    theme.color_mode)
                        COMPREPLY=( $(compgen -W "${color_modes}" -- "${cur}") )
                        return 0
                        ;;
                    features.sprints|features.epics|features.phases)
                        COMPREPLY=( $(compgen -W "${feature_flags}" -- "${cur}") )
                        return 0
                        ;;
                    paths.backlog|paths.sprints)
                        COMPREPLY=( $(compgen -d -- "${cur}") )
                        return 0
                        ;;
                esac
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_sprint_create(script: &str) -> String {
    let old = r#"        kanban__subcmd__sprint__subcmd__create)
            opts="-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --number)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --headline)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --start)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --end)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = format!(
        r#"        kanban__subcmd__sprint__subcmd__create)
            opts="-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]"
            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
                return 0
            fi
            case "${{prev}}" in
                --number)
                    COMPREPLY=()
                    return 0
                    ;;
                --headline)
                    COMPREPLY=()
                    return 0
                    ;;
                --start)
                    COMPREPLY=( $(compgen -W "{date_placeholder}" -- "${{cur}}") )
                    return 0
                    ;;
                --end)
                    COMPREPLY=( $(compgen -W "{date_placeholder}" -- "${{cur}}") )
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
            return 0"#,
        date_placeholder = BASH_DATE_PLACEHOLDER,
    );
    if script.contains(old) {
        script.replacen(old, &new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_web_log(script: &str) -> String {
    let old = r#"        kanban__subcmd__web__subcmd__log)
            opts="-f -h --lines --follow --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --lines)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__web__subcmd__log)
            opts="-f -h --lines --follow --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --lines)
                    COMPREPLY=()
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

pub(crate) fn inject_bash_story_plan(script: &str) -> String {
    let replacement = r#"        kanban__subcmd__story__subcmd__plan)
             opts="-h --sprint --format --help <ID> [REPO_ROOT]"
              if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                  local -a matches=()
                  local id
                  while IFS= read -r id; do
                      [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                  done < <(kanban list-ids stories 2>/dev/null)
                  COMPREPLY=( "${matches[@]}" )
                  return 0
              fi
             case "${prev}" in
                 --sprint)
                     COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0
             ;;
"#;
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__plan", replacement)
}

pub(crate) fn inject_bash_story_move_status(script: &str) -> String {
    let story_statuses = CANONICAL_STORY_STATUSES.join(" ");
    let replacement = r#"        kanban__subcmd__story__subcmd__move)
             opts="-a -h --assignee --format --help <ID> <STATUS> [REPO_ROOT]"
             story_statuses="__KANBAN_STORY_STATUSES__"
              if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                  local -a matches=()
                  local id
                  while IFS= read -r id; do
                      [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                  done < <(kanban list-ids stories 2>/dev/null)
                  COMPREPLY=( "${matches[@]}" )
                  return 0
              fi
             if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "${story_statuses}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --assignee)
                     COMPREPLY=()
                     return 0
                     ;;
                 -a)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0
             ;;
"#;
    let replacement = replacement.replace("__KANBAN_STORY_STATUSES__", &story_statuses);
    replace_bash_case_block(script, "kanban__subcmd__story__subcmd__move", &replacement)
}

pub(crate) fn inject_bash_task_add_status(script: &str) -> String {
    let task_statuses = CANONICAL_TASK_STATUSES.join(" ");
    let replacement = r#"        kanban__subcmd__task__subcmd__add)
             opts="-h --title --status --tags --description --format --help <STORY_ID> [REPO_ROOT]"
             task_statuses="__KANBAN_TASK_STATUSES__"
              if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                  local -a matches=()
                  local id
                  while IFS= read -r id; do
                      [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                  done < <(kanban list-ids stories 2>/dev/null)
                  COMPREPLY=( "${matches[@]}" )
                  return 0
              fi
             case "${prev}" in
                 --title)
                     COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=( $(compgen -W "${task_statuses}" -- "${cur}") )
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0
             ;;
"#;
    let replacement = replacement.replace("__KANBAN_TASK_STATUSES__", &task_statuses);
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__add", &replacement)
}

pub(crate) fn inject_bash_task_update_status(script: &str) -> String {
    let task_statuses = CANONICAL_TASK_STATUSES.join(" ");
    let replacement = r#"        kanban__subcmd__task__subcmd__update)
             opts="-h --title --status --tags --description --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]"
             task_statuses="__KANBAN_TASK_STATUSES__"
              if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                  local -a matches=()
                  local id
                  while IFS= read -r id; do
                      [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                  done < <(kanban list-ids stories 2>/dev/null)
                   COMPREPLY=( "${matches[@]}" )
                   return 0
               fi
               if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                   local resolved_story
                   resolved_story=$(_kanban_resolve_story_id "${prev}")
                   if [[ -n "${resolved_story}" ]] ; then
                       local -a matches=()
                       local id
                       while IFS= read -r id; do
                           [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                       done < <(kanban list-task-ids "${resolved_story}" 2>/dev/null)
                       COMPREPLY=( "${matches[@]}" )
                   else
                       COMPREPLY=()
                   fi
                   return 0
               fi
             case "${prev}" in
                  --title)
                      COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=( $(compgen -W "${task_statuses}" -- "${cur}") )
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0
             ;;
"#;
    let replacement = replacement.replace("__KANBAN_TASK_STATUSES__", &task_statuses);
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__update", &replacement)
}

pub(crate) fn inject_bash_task_delete(script: &str) -> String {
    let replacement = r#"        kanban__subcmd__task__subcmd__delete)
             opts="-h --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]"
              if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                  local -a matches=()
                  local id
                  while IFS= read -r id; do
                      [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                  done < <(kanban list-ids stories 2>/dev/null)
                  COMPREPLY=( "${matches[@]}" )
                  return 0
              fi
              if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                  local resolved_story
                  resolved_story=$(_kanban_resolve_story_id "${prev}")
                  if [[ -n "${resolved_story}" ]] ; then
                      local -a matches=()
                      local id
                      while IFS= read -r id; do
                          [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                      done < <(kanban list-task-ids "${resolved_story}" 2>/dev/null)
                      COMPREPLY=( "${matches[@]}" )
                  else
                      COMPREPLY=()
                  fi
                  return 0
              fi
             case "${prev}" in
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0
             ;;
"#;
    replace_bash_case_block(script, "kanban__subcmd__task__subcmd__delete", replacement)
}

/// Enhance the bash completion script with dynamic sprint name, story ID,
/// and doctor fix target completions.
pub(crate) fn enhance_bash_completion(script: &str) -> String {
    let script = inject_bash_doctor_command_or_repo_root(script);
    let script = inject_bash_sprint_create(&script);
    let script = inject_bash_phase_show(&script);
    let script = inject_bash_story_list(&script);
    let script = inject_bash_list_task_ids(&script);
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

/// Shared bash helper appended to the completion script. Returns success when
/// `$1` contains `$2` as a case-insensitive substring (empty needle matches all).
/// Uses `tr` so it works on bash 3.2 (the macOS system bash) as well as bash 4+.
pub(crate) const BASH_CI_MATCH_HELPER: &str = r#"
# Case-insensitive substring match used by kanban dynamic ID completions.
_kanban_ci_match() {
    local hay needle
    needle="$2"
    [[ -z "$needle" ]] && return 0
    hay=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
    needle=$(printf '%s' "$needle" | tr '[:upper:]' '[:lower:]')
    [[ "$hay" == *"$needle"* ]]
}
"#;

pub(crate) const BASH_RESOLVE_STORY_ID_HELPER: &str = r#"
# Resolve a task's parent story only when it matches a real story ID exactly.
_kanban_resolve_story_id() {
    local candidate id
    candidate="$1"
    [[ -z "$candidate" ]] && return 1
    while IFS= read -r id; do
        [[ "$id" == "$candidate" ]] && printf '%s\n' "$id" && return 0
    done < <(kanban list-ids stories 2>/dev/null)
    return 1
}
"#;

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
    format!("{script}{BASH_CI_MATCH_HELPER}{BASH_RESOLVE_STORY_ID_HELPER}")
}

/// Register the documented `kb` alias for the same completion function as
/// `kanban`, mirroring clap_complete's bash-version-aware `complete` call.
pub(crate) fn append_bash_kb_alias(script: &str) -> String {
    let registration = r#"
if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _kanban -o nosort -o bashdefault -o default kb
else
    complete -F _kanban -o bashdefault -o default kb
fi
"#;
    format!("{script}{registration}")
}

#[allow(dead_code)]
pub(crate) fn inject_bash_story_update(_script: &str) -> String {
    String::new()
}

/*
        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --type)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --status)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --epic)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=()
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids epics 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids stories 2>/dev/null)
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --type)
                    COMPREPLY=( $(compgen -W "user-story epic" -- "${cur}") )
                    return 0
                    ;;
                --status)
                    COMPREPLY=( $(compgen -W "draft backlog ready todo in-progress ready-for-qa blocked done dropped" -- "${cur}") )
                    return 0
                    ;;
                --epic)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=( $(compgen -W "$(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[]",' | tr '[:space:]' ' ')" -- "${cur}") )
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
*/

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

    let replacement = r#"        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=()
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids epics 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids stories 2>/dev/null)
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --type)
                    COMPREPLY=( $(compgen -W "user-story epic" -- "${cur}") )
                    return 0
                    ;;
                --status)
                    COMPREPLY=( $(compgen -W "draft backlog ready todo in-progress ready-for-qa blocked done dropped" -- "${cur}") )
                    return 0
                    ;;
                --epic)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=( $(compgen -W "$(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[],\"' | tr '[:space:]' ' ')" -- "${cur}") )
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=( $(compgen -W "human json" -- "${cur}") )
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
"#;

    let mut result =
        String::with_capacity(script.len() + replacement.len().saturating_sub(end - start));
    result.push_str(&script[..start]);
    result.push_str(replacement);
    result.push_str(&script[end..]);
    result
}
