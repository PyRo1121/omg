_omg_completions() {
    local cur last
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    last="${COMP_WORDS[COMP_CWORD-1]}"

    case "$last" in
        install|i|remove|r|info|use|ls|list|which)
            local suggestions=$(omg complete --shell bash --current "$cur" --last "$last")
            COMPREPLY=( $(compgen -W "$suggestions" -- "$cur") )
            return 0
            ;;
    esac

    if [[ $COMP_CWORD -eq 1 ]]; then
        local commands="search install remove update info clean explicit sync use list hook daemon config completions which status doctor audit run new tool env history rollback dash help"
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    fi
}

complete -F _omg_completions omg
