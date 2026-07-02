__KANBAN_SECTION__INJECT_BASH_PHASE_SHOW_REPLACEMENT__START__
        kanban__subcmd__phase__subcmd__show)
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
__KANBAN_SECTION__INJECT_BASH_PHASE_SHOW_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_STORY_LIST_REPLACEMENT__START__
        kanban__subcmd__story__subcmd__list)
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
__KANBAN_SECTION__INJECT_BASH_STORY_LIST_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_LIST_TASK_IDS_REPLACEMENT__START__
        kanban__subcmd__list__subcmd__task__subcmd__ids)
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
__KANBAN_SECTION__INJECT_BASH_LIST_TASK_IDS_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_DOCTOR_FIX_TARGET_OLD__START__
        kanban__subcmd__doctor__subcmd__fix)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_DOCTOR_FIX_TARGET_OLD__END__
__KANBAN_SECTION__INJECT_BASH_DOCTOR_FIX_TARGET_NEW__START__
        kanban__subcmd__doctor__subcmd__fix)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_DOCTOR_FIX_TARGET_NEW__END__
__KANBAN_SECTION__INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_OLD__START__
        kanban__subcmd__doctor)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_OLD__END__
__KANBAN_SECTION__INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_NEW__START__
        kanban__subcmd__doctor)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_DOCTOR_COMMAND_OR_REPO_ROOT_NEW__END__
__KANBAN_SECTION__INJECT_BASH_CONFIG_GET_OLD__START__
        kanban__subcmd__config__subcmd__get)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_CONFIG_GET_OLD__END__
__KANBAN_SECTION__INJECT_BASH_CONFIG_GET_NEW__START__
        kanban__subcmd__config__subcmd__get)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_CONFIG_GET_NEW__END__
__KANBAN_SECTION__INJECT_BASH_CONFIG_SET_OLD__START__
        kanban__subcmd__config__subcmd__set)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_CONFIG_SET_OLD__END__
__KANBAN_SECTION__INJECT_BASH_CONFIG_SET_NEW__START__
        kanban__subcmd__config__subcmd__set)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_CONFIG_SET_NEW__END__
__KANBAN_SECTION__INJECT_BASH_SPRINT_CREATE_OLD__START__
        kanban__subcmd__sprint__subcmd__create)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_SPRINT_CREATE_OLD__END__
__KANBAN_SECTION__INJECT_BASH_SPRINT_CREATE_NEW__START__
        kanban__subcmd__sprint__subcmd__create)
            opts="-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --number)
                    COMPREPLY=()
                    return 0
                    ;;
                --headline)
                    COMPREPLY=()
                    return 0
                    ;;
                --start)
                    COMPREPLY=( $(compgen -W "__KANBAN_DATE_PLACEHOLDER__" -- "${cur}") )
                    return 0
                    ;;
                --end)
                    COMPREPLY=( $(compgen -W "__KANBAN_DATE_PLACEHOLDER__" -- "${cur}") )
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
__KANBAN_SECTION__INJECT_BASH_SPRINT_CREATE_NEW__END__
__KANBAN_SECTION__INJECT_BASH_WEB_LOG_OLD__START__
        kanban__subcmd__web__subcmd__log)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_WEB_LOG_OLD__END__
__KANBAN_SECTION__INJECT_BASH_WEB_LOG_NEW__START__
        kanban__subcmd__web__subcmd__log)
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
            return 0
__KANBAN_SECTION__INJECT_BASH_WEB_LOG_NEW__END__
__KANBAN_SECTION__INJECT_BASH_STORY_PLAN_REPLACEMENT__START__
        kanban__subcmd__story__subcmd__plan)
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
__KANBAN_SECTION__INJECT_BASH_STORY_PLAN_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_STORY_MOVE_STATUS_REPLACEMENT__START__
        kanban__subcmd__story__subcmd__move)
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
__KANBAN_SECTION__INJECT_BASH_STORY_MOVE_STATUS_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_TASK_ADD_STATUS_REPLACEMENT__START__
        kanban__subcmd__task__subcmd__add)
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
__KANBAN_SECTION__INJECT_BASH_TASK_ADD_STATUS_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_TASK_UPDATE_STATUS_REPLACEMENT__START__
        kanban__subcmd__task__subcmd__update)
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
__KANBAN_SECTION__INJECT_BASH_TASK_UPDATE_STATUS_REPLACEMENT__END__
__KANBAN_SECTION__INJECT_BASH_TASK_DELETE_REPLACEMENT__START__
        kanban__subcmd__task__subcmd__delete)
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
__KANBAN_SECTION__INJECT_BASH_TASK_DELETE_REPLACEMENT__END__
__KANBAN_SECTION__BASH_CI_MATCH_HELPER__START__

# Case-insensitive substring match used by kanban dynamic ID completions.
_kanban_ci_match() {
    local hay needle
    needle="$2"
    [[ -z "$needle" ]] && return 0
    hay=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
    needle=$(printf '%s' "$needle" | tr '[:upper:]' '[:lower:]')
    [[ "$hay" == *"$needle"* ]]
}
__KANBAN_SECTION__BASH_CI_MATCH_HELPER__END__
__KANBAN_SECTION__BASH_RESOLVE_STORY_ID_HELPER__START__

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
__KANBAN_SECTION__BASH_RESOLVE_STORY_ID_HELPER__END__
__KANBAN_SECTION__APPEND_BASH_KB_ALIAS_REGISTRATION__START__

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _kanban -o nosort -o bashdefault -o default kb
else
    complete -F _kanban -o bashdefault -o default kb
fi
__KANBAN_SECTION__APPEND_BASH_KB_ALIAS_REGISTRATION__END__
__KANBAN_SECTION__INJECT_BASH_STORY_UPDATE_DYNAMIC_REPLACEMENT__START__
        kanban__subcmd__story__subcmd__update)
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
                    COMPREPLY=( $(compgen -W "draft backlog ready planned todo in-progress ready-for-qa done blocked dropped" -- "${cur}") )
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
__KANBAN_SECTION__INJECT_BASH_STORY_UPDATE_DYNAMIC_REPLACEMENT__END__
