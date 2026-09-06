#!/usr/bin/env bash
# Exercise the real installer functions with isolated transport/tool fixtures.
set -euo pipefail
cd "$(dirname "$0")/.."
task_dir=$(mktemp -d)
trap 'rm -rf "$task_dir"' EXIT
sed '$d' install.sh > "$task_dir/functions.sh"
for scenario in missing rejected wrong_tag accepted; do
  (
    set --
    source "$task_dir/functions.sh"
    trap - EXIT
    scenario_dir="$task_dir/$scenario"
    mkdir -p "$scenario_dir"
    INSTALL_DIR="$scenario_dir/bin"
    OMG_VERSION=v1.2.3
    for name in start_spinner stop_spinner fail_spinner header info success; do
      eval "$name() { :; }"
    done
    check_runtime_dependencies() { return 0; }
    detect_os() { echo linux; }
    detect_distro() { echo arch; }
    detect_arch() { echo x86_64; }
    fetch_release_json() {
      local version=v1.2.3
      [[ "$scenario" != wrong_tag ]] || version=v1.2.2
      printf '{"tag_name":"%s","assets":[{"browser_download_url":"https://github.com/PyRo1121/omg/releases/download/%s/omg-%s-x86_64-linux-arch.tar.gz"}]}' "$version" "$version" "$version"
    }
    curl() {
      if [[ "$2" == *.sha256 ]]; then
        printf '%064d  archive\n' 0 > "$4"
      else
        printf 'fixture' > "$4"
      fi
    }
    calculate_sha256() { printf '%064d\n' 0; }
    command() {
      if [[ "$scenario" == missing && "$*" == '-v gh' ]]; then return 1; fi
      builtin command "$@"
    }
    gh() {
      printf '%s\n' "$@" > "$scenario_dir/gh-args"
      [[ "$scenario" != rejected ]]
    }
    tar() {
      touch "$scenario_dir/extracted"
      printf '#!/bin/sh\nexit 0\n' > "$tmp_dir/omg"
    }
    install_binary() { mkdir -p "$INSTALL_DIR"; cp "$1" "$2"; }
    if install_from_release; then status=0; else status=$?; fi
    if [[ "$scenario" == accepted ]]; then
      [[ "$status" == 0 && -f "$INSTALL_DIR/omg" ]]
      grep -Fx -- '--source-ref' "$scenario_dir/gh-args"
      grep -Fx 'refs/tags/v1.2.3' "$scenario_dir/gh-args"
      grep -Fx 'PyRo1121/omg/.github/workflows/release.yml' "$scenario_dir/gh-args"
    else
      [[ "$status" != 0 && ! -e "$scenario_dir/extracted" && ! -e "$INSTALL_DIR/omg" ]]
    fi
  )
done
# Piped definitions run in an attacker-controlled checkout, even with the old
# auto-detection bait. They must never qualify as an explicit source install.
mkdir -p "$task_dir/ambient"
printf '[package]\nname = "omg"\n' > "$task_dir/ambient/Cargo.toml"
{ cat "$task_dir/functions.sh"; printf '\n[[ "$IS_SOURCE_INSTALL" == false ]]\n'; } | (cd "$task_dir/ambient"; bash)
printf 'Installer security scenarios passed\n'
