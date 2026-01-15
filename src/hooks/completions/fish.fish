function __omg_dynamic_complete
    set -l cur (commandline -ct)
    set -l cmd (commandline -opc)
    set -l last ""
    if test (count $cmd) -gt 0
        set last $cmd[-1]
    end

    if test -n "$last"
        omg complete --shell fish --current "$cur" --last "$last"
    end
end

function __omg_needs_command
    set -l cmd (commandline -opc)
    test (count $cmd) -le 1
end

function __omg_needs_suggestions
    set -l cmd (commandline -opc)
    if test (count $cmd) -lt 2
        return 1
    end

    set -l last $cmd[-1]
    switch $last
        case install i remove r info use ls list which
            return 0
    end

    return 1
end

complete -c omg -f -n '__omg_needs_command' -a 'search install remove update info clean explicit sync use list hook daemon config completions which status doctor audit run new tool env history rollback dash help'
complete -c omg -f -n '__omg_needs_suggestions' -a '(__omg_dynamic_complete)'
