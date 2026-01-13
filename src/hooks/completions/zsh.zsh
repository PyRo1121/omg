#compdef omg

_omg() {
    local -a commands
    local -a subcommands
    
    # Check if we should use dynamic completion
    local last_word="${words[$CURRENT-1]}"
    local current_word="${words[$CURRENT]}"
    
    case "$last_word" in
        install|i|remove|r|info|use|ls|list|which)
            local -a suggestions
            suggestions=(${(f)"$(omg complete --shell zsh --current "$current_word" --last "$last_word")"})
            if [[ -n "$suggestions" ]]; then
                _values 'suggestions' $suggestions
                return 0
            fi
            ;;
    esac

    # Fallback to standard clap-generated completions (simplified)
    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            commands=(
                'search:Search for packages'
                'install:Install packages'
                'remove:Remove packages'
                'update:Update all packages'
                'info:Show package information'
                'clean:Clean up orphan packages'
                'explicit:List explicitly installed packages'
                'use:Switch runtime version'
                'list:List installed versions'
                'hook:Print shell hook'
                'daemon:Start the OMG daemon'
                'config:Get or set configuration'
                'completions:Generate shell completions'
                'which:Show which version of a runtime would be used'
                'status:Show system status'
                'audit:Perform a security audit'
                'env:Environment management'
            )
            _describe -t commands 'omg commands' commands
            ;;
    esac
}

_omg "$@"
