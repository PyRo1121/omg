#compdef omg

_omg() {
    local -a commands suggestions

    # Get current context
    local last_word="${words[$CURRENT-1]}"
    local current_word="${words[$CURRENT]}"
    local full_line="${BUFFER}"

    # Dynamic completion for package names and other contexts.
    # `suggestions` resolves through zsh dynamic scoping to the caller's array.
    _omg_dynamic_complete() {
        local line
        omg complete --shell zsh --current "$current_word" \
            --last "$last_word" --full "$full_line" 2>/dev/null \
            | while IFS= read -r line; do
                suggestions+=("$line")
            done
    }

    case "$last_word" in
        install|i|remove|r|info|use|ls|list|which|tool|env|run|new)
            _omg_dynamic_complete
            if [[ ${#suggestions[@]} -gt 0 ]]; then
                compadd -U -a suggestions
                return 0
            fi
            ;;
    esac

    # Fallback to dynamic completion for any context beyond the first command
    if [[ $CURRENT -gt 2 ]]; then
        _omg_dynamic_complete
        if [[ ${#suggestions[@]} -gt 0 ]]; then
            compadd -U -a suggestions
            return 0
        fi
    fi

    # Main command completion
    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            # shellcheck disable=SC2034  # consumed by _describe below, which
            # takes the variable NAME and reads it out-of-band from shellcheck.
            commands=(
                'search:Search for packages'
                'install:Install packages (supports tab completion for package names)'
                'remove:Remove packages (supports tab completion for installed packages)'
                'update:Update all packages'
                'info:Show package information (supports tab completion)'
                'why:Explain why a package is installed'
                'outdated:Show what packages would be updated'
                'size:Show disk usage by packages'
                'blame:Show when and why a package was installed'
                'diff:Compare two environment lock files'
                'snapshot:Create or restore environment snapshots'
                'ci:Generate CI/CD configuration'
                'migrate:Cross-distro migration tools'
                'clean:Clean up orphan packages'
                'explicit:List explicitly installed packages'
                'sync:Sync package databases'
                'hooks:Manage Git hooks for environment synchronization'
                'workspace:Workspace management for monorepos'
                'privacy:Privacy and telemetry controls'
                'generate-man:Generate man pages'
                'daemon-status:Show detailed daemon status'
                'self-update:Update OMG to the latest version'
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
                's:Alias for search'
                'i:Alias for install'
                'r:Alias for remove'
                'u:Alias for update'
                'sy:Alias for use'
                'ls:Alias for list'
                'create:Alias for new'
                'd:Alias for dash'
                'up:Alias for self-update'
                'help:Show help'
            )
            _describe -t commands 'omg commands' commands
            ;;
    esac
}

_omg "$@"
