__KANBAN_SECTION__ZSH_DYNAMIC_HELPERS__START__

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
__KANBAN_SECTION__ZSH_DYNAMIC_HELPERS__END__
__KANBAN_SECTION__ZSH_KB_ALIAS_REGISTRATION__START__

if [ "$funcstack[1]" != "_kanban" ]; then
    compdef _kanban kb
fi
__KANBAN_SECTION__ZSH_KB_ALIAS_REGISTRATION__END__
