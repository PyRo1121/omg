_omg_completions() {
    local cur last full suggestions suggestion
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    last="${COMP_WORDS[COMP_CWORD - 1]}"
    full="${COMP_LINE}"

    # Main command completion (visible commands + their visible aliases)
    if [[ $COMP_CWORD -eq 1 ]]; then
        local commands="search s install i remove r update u info why outdated size blame diff snapshot ci migrate clean explicit sync use sy list ls hook hooks workspace daemon config privacy generate-man daemon-status completions which status doctor audit run new create tool env team container license fleet enterprise history rollback dash d stats metrics self-update up init help"
        COMPREPLY=($(compgen -W "$commands" -- "$cur"))
        return 0
    fi

    if [[ $COMP_CWORD -gt 1 ]]; then
        suggestions=$(omg complete --shell bash --current "$cur" --last "$last" --full "$full" 2>/dev/null) || return 0
        while IFS= read -r suggestion; do
            if [[ -n "$suggestion" ]]; then
                COMPREPLY+=("$suggestion")
            fi
        done <<< "$suggestions"
    fi
}

complete -F _omg_completions omg
