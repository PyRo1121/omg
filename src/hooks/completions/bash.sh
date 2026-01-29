_omg_completions() {
    local cur last full
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    last="${COMP_WORDS[COMP_CWORD-1]}"
    full="${COMP_LINE}"

    # Dynamic completion for package names and more
    case "$last" in
        install|i|remove|r|info|use|ls|list|which|tool|env|run|new)
            local suggestions=$(omg complete --shell bash --current "$cur" --last "$last" --full "$full" 2>/dev/null)
            if [[ -n "$suggestions" ]]; then
                COMPREPLY=( $(compgen -W "$suggestions" -- "$cur") )
                return 0
            fi
            ;;
    esac

    # Main command completion
    if [[ $COMP_CWORD -eq 1 ]]; then
        local commands="search install remove update info why outdated pin size blame diff snapshot ci migrate clean explicit sync use list hook daemon config completions which status doctor audit run new tool env team container license fleet enterprise history rollback dash stats metrics init help"
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
        return 0
    fi

    # Fallback to dynamic completion for any context
    if [[ $COMP_CWORD -gt 1 ]]; then
        local suggestions=$(omg complete --shell bash --current "$cur" --last "$last" --full "$full" 2>/dev/null)
        if [[ -n "$suggestions" ]]; then
            COMPREPLY=( $(compgen -W "$suggestions" -- "$cur") )
            return 0
        fi
    fi
}

complete -F _omg_completions omg
