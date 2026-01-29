#compdef omg

_omg() {
    local -a commands
    local -a subcommands

    # Get current context
    local last_word="${words[$CURRENT-1]}"
    local current_word="${words[$CURRENT]}"
    local full_line="${BUFFER}"

    # Dynamic completion for package names and other contexts
    case "$last_word" in
        install|i|remove|r|info|use|ls|list|which|tool|env|run|new)
            local -a suggestions
            suggestions=(${(f)"$(omg complete --shell zsh --current "$current_word" --last "$last_word" --full "$full_line" 2>/dev/null)"})
            if [[ ${#suggestions[@]} -gt 0 ]]; then
                compadd -a suggestions
                return 0
            fi
            ;;
    esac

    # Fallback to dynamic completion for any context beyond the first command
    if [[ $CURRENT -gt 2 ]]; then
        local -a suggestions
        suggestions=(${(f)"$(omg complete --shell zsh --current "$current_word" --last "$last_word" --full "$full_line" 2>/dev/null)"})
        if [[ ${#suggestions[@]} -gt 0 ]]; then
            compadd -a suggestions
            return 0
        fi
    fi

    # Main command completion
    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            commands=(
                'search:Search for packages'
                'install:Install packages (supports tab completion for package names)'
                'remove:Remove packages (supports tab completion for installed packages)'
                'update:Update all packages'
                'info:Show package information (supports tab completion)'
                'why:Explain why a package is installed'
                'outdated:Show what packages would be updated'
                'pin:Pin a package to prevent updates'
                'size:Show disk usage by packages'
                'blame:Show when and why a package was installed'
                'diff:Compare two environment lock files'
                'snapshot:Create or restore environment snapshots'
                'ci:Generate CI/CD configuration'
                'migrate:Cross-distro migration tools'
                'clean:Clean up orphan packages'
                'explicit:List explicitly installed packages'
                'sync:Sync package databases'
                'use:Switch runtime version'
                'list:List installed versions'
                'hook:Print shell hook'
                'daemon:Start the OMG daemon'
                'config:Get or set configuration'
                'completions:Generate shell completions'
                'which:Show which version of a runtime would be used'
                'status:Show system status'
                'doctor:Check system health'
                'audit:Perform a security audit'
                'run:Run project scripts'
                'new:Create a new project'
                'tool:Manage dev tools'
                'env:Environment management'
                'team:Team collaboration tools'
                'container:Container integration'
                'license:License management'
                'fleet:Fleet management'
                'enterprise:Enterprise features'
                'history:View package transaction history'
                'rollback:Roll back to a previous system state'
                'dash:Interactive TUI dashboard'
                'stats:Usage statistics'
                'metrics:Performance metrics'
                'init:Initialize OMG configuration'
                'help:Show help'
            )
            _describe -t commands 'omg commands' commands
            ;;
    esac
}

_omg "$@"
