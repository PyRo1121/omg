#!/usr/bin/env zsh
set -eu

bash --noprofile --norc -s "${0:A:h}/../src/hooks/completions/bash.sh" <<'BASH'
set -eu
source "$1"
omg() {
    case "$5" in
        frfx) printf '%s\n' firefox ;;
        gt) printf '%s\n' git ;;
        failure) printf '%s\n' firefox; return 1 ;;
    esac
}
for query in frfx gt unmatched failure; do
    COMP_WORDS=(omg install "$query")
    COMP_CWORD=2
    COMP_LINE="omg install $query"
    _omg_completions
    printf 'Bash %s => %s\n' "$query" "${COMPREPLY[*]-}"
    case "$query" in
        frfx) [[ "${COMPREPLY[*]}" == firefox ]] ;;
        gt) [[ "${COMPREPLY[*]}" == git ]] ;;
        *) [[ ${#COMPREPLY[@]} -eq 0 ]] ;;
    esac
done
BASH

zmodload zsh/zpty
zmodload zsh/system
owner=$sysparams[pid]
cache_root=${XDG_CACHE_HOME:-$HOME/.cache}
mkdir -p "$cache_root"
fixture=$(mktemp -d "$cache_root/omg-zsh-completion.XXXXXXXX")
cleanup() {
    [[ $sysparams[pid] == $owner ]] || return 0
    if zpty -t child 2>/dev/null; then
        zpty -d child
    fi
    rm -rf -- "$fixture"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir "$fixture/fpath"
cp "${0:A:h}/../src/hooks/completions/zsh.zsh" "$fixture/fpath/_omg"
cat > "$fixture/.zshrc" <<'RC'
fpath=("$ZDOTDIR/fpath" $fpath)
autoload -Uz compinit
compinit -D
PROMPT='READY> '
omg() {
    case "$5" in
        frfx) print -r -- firefox ;;
        gt) print -r -- git ;;
    esac
}
capture_buffer() {
    print -r -- "CAPTURE:$BUFFER:END"
    zle redisplay
}
zle -N capture_buffer
bindkey '^X' capture_buffer
RC
export ZDOTDIR=$fixture TERM=xterm
zpty child zsh -d -i
zpty -r child output '*READY>*'
zpty -w -n child $'omg install frfx\t\C-x'
zpty -r child output '*CAPTURE:*:END*'
print -r -- "$output"
[[ "$output" == *'CAPTURE:omg install firefox :END'* ]]
zpty -w -n child $'\C-a\C-komg install gt\t\C-x'
zpty -r child output '*CAPTURE:*:END*'
print -r -- "$output"
[[ "$output" == *'CAPTURE:omg install git :END'* ]]
zpty -w -n child $'\C-a\C-komg install zzzz-unmatched\t\C-x'
zpty -r child output '*CAPTURE:*:END*'
print -r -- "$output"
[[ "$output" == *'CAPTURE:omg install zzzz-unmatched:END'* ]]
