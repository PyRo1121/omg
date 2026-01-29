function __omg_dynamic_complete
    set -l cur (commandline -ct)
    set -l cmd (commandline -opc)
    set -l full (commandline -p)
    set -l last ""

    if test (count $cmd) -gt 0
        set last $cmd[-1]
    end

    if test -n "$last"
        omg complete --shell fish --current "$cur" --last "$last" --full "$full" 2>/dev/null
    end
end

function __omg_needs_command
    set -l cmd (commandline -opc)
    test (count $cmd) -le 1
end

function __omg_needs_package_suggestions
    set -l cmd (commandline -opc)
    if test (count $cmd) -lt 2
        return 1
    end

    set -l last $cmd[-1]
    switch $last
        case install i remove r info use ls list which tool env run new
            return 0
    end

    return 1
end

# Main commands
complete -c omg -f -n '__omg_needs_command' -a 'search' -d 'Search for packages'
complete -c omg -f -n '__omg_needs_command' -a 'install' -d 'Install packages (tab completes package names)'
complete -c omg -f -n '__omg_needs_command' -a 'remove' -d 'Remove packages (tab completes installed packages)'
complete -c omg -f -n '__omg_needs_command' -a 'update' -d 'Update all packages'
complete -c omg -f -n '__omg_needs_command' -a 'info' -d 'Show package information'
complete -c omg -f -n '__omg_needs_command' -a 'why' -d 'Explain why a package is installed'
complete -c omg -f -n '__omg_needs_command' -a 'outdated' -d 'Show what packages would be updated'
complete -c omg -f -n '__omg_needs_command' -a 'pin' -d 'Pin a package'
complete -c omg -f -n '__omg_needs_command' -a 'size' -d 'Show disk usage'
complete -c omg -f -n '__omg_needs_command' -a 'blame' -d 'Show install history'
complete -c omg -f -n '__omg_needs_command' -a 'diff' -d 'Compare environments'
complete -c omg -f -n '__omg_needs_command' -a 'snapshot' -d 'Manage snapshots'
complete -c omg -f -n '__omg_needs_command' -a 'ci' -d 'CI/CD tools'
complete -c omg -f -n '__omg_needs_command' -a 'migrate' -d 'Migration tools'
complete -c omg -f -n '__omg_needs_command' -a 'clean' -d 'Clean up packages'
complete -c omg -f -n '__omg_needs_command' -a 'explicit' -d 'List explicitly installed'
complete -c omg -f -n '__omg_needs_command' -a 'sync' -d 'Sync package databases'
complete -c omg -f -n '__omg_needs_command' -a 'use' -d 'Switch runtime version'
complete -c omg -f -n '__omg_needs_command' -a 'list' -d 'List installed versions'
complete -c omg -f -n '__omg_needs_command' -a 'hook' -d 'Print shell hook'
complete -c omg -f -n '__omg_needs_command' -a 'daemon' -d 'Daemon management'
complete -c omg -f -n '__omg_needs_command' -a 'config' -d 'Configuration'
complete -c omg -f -n '__omg_needs_command' -a 'completions' -d 'Generate completions'
complete -c omg -f -n '__omg_needs_command' -a 'which' -d 'Show active version'
complete -c omg -f -n '__omg_needs_command' -a 'status' -d 'System status'
complete -c omg -f -n '__omg_needs_command' -a 'doctor' -d 'Check system health'
complete -c omg -f -n '__omg_needs_command' -a 'audit' -d 'Security audit'
complete -c omg -f -n '__omg_needs_command' -a 'run' -d 'Run project scripts'
complete -c omg -f -n '__omg_needs_command' -a 'new' -d 'Create new project'
complete -c omg -f -n '__omg_needs_command' -a 'tool' -d 'Manage dev tools'
complete -c omg -f -n '__omg_needs_command' -a 'env' -d 'Environment management'
complete -c omg -f -n '__omg_needs_command' -a 'team' -d 'Team collaboration'
complete -c omg -f -n '__omg_needs_command' -a 'container' -d 'Container integration'
complete -c omg -f -n '__omg_needs_command' -a 'license' -d 'License management'
complete -c omg -f -n '__omg_needs_command' -a 'fleet' -d 'Fleet management'
complete -c omg -f -n '__omg_needs_command' -a 'enterprise' -d 'Enterprise features'
complete -c omg -f -n '__omg_needs_command' -a 'history' -d 'Transaction history'
complete -c omg -f -n '__omg_needs_command' -a 'rollback' -d 'Rollback to previous state'
complete -c omg -f -n '__omg_needs_command' -a 'dash' -d 'Interactive dashboard'
complete -c omg -f -n '__omg_needs_command' -a 'stats' -d 'Usage statistics'
complete -c omg -f -n '__omg_needs_command' -a 'metrics' -d 'Performance metrics'
complete -c omg -f -n '__omg_needs_command' -a 'init' -d 'Initialize configuration'
complete -c omg -f -n '__omg_needs_command' -a 'help' -d 'Show help'

# Dynamic package/runtime/tool suggestions
complete -c omg -f -n '__omg_needs_package_suggestions' -a '(__omg_dynamic_complete)'
