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
    if [[ "$scenario" == wrong_tag ]]; then
      resolve_version() { printf 'v1.2.2\n'; }
    fi
    curl() {
      local url="${@: -1}"
      if [[ "$url" == *.sha256 ]]; then
        printf '%064d  archive\n' 0
      elif [[ "$url" == "$RELEASES_BASE_URL/omg-v1.2.3-x86_64-linux-arch.tar.gz" ]]; then
        printf 'fixture'
      else
        printf 'Unexpected fixture URL: %s\n' "$url" >&2
        return 1
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
# Replace an initially absent destination with a directory after its pre-check,
# during staging. The real final rename must fail without touching its child.
for rename_platform in native darwin; do
  (
    set --
    source "$task_dir/functions.sh"
    trap - EXIT
    INSTALL_DIR="$task_dir/raced-$rename_platform"
    mkdir -p "$INSTALL_DIR"
    printf 'new binary\n' > "$INSTALL_DIR/source"
    if [[ "$rename_platform" == darwin ]]; then
      uname() { printf 'Darwin\n'; }
    fi
    rename_install_binary "$INSTALL_DIR/source" "$INSTALL_DIR/control"
    [[ $(cat "$INSTALL_DIR/control") == 'new binary' ]]
    printf 'updated binary\n' > "$INSTALL_DIR/source"
    rename_install_binary "$INSTALL_DIR/source" "$INSTALL_DIR/control"
    [[ $(cat "$INSTALL_DIR/control") == 'updated binary' ]]
    printf 'new binary\n' > "$INSTALL_DIR/source"
    cp() {
      command cp "$@"
      mkdir "$INSTALL_DIR/omg"
      printf 'untouched\n' > "$INSTALL_DIR/omg/omg"
    }
    if (install_binary "$INSTALL_DIR/source" "$INSTALL_DIR/omg"); then
      printf 'Installer followed a directory swapped in during staging\n' >&2
      exit 1
    fi
    [[ $(cat "$INSTALL_DIR/omg/omg") == untouched ]]
    [[ $(find "$INSTALL_DIR/omg" -type f | wc -l) -eq 1 ]]
    printf 'PASS: %s direct rename and directory-swap controls\n' "$rename_platform"
  )
done
# The final move must replace a destination symlink, never install inside its
# directory target. Exercise the real install function, including regular updates.
(
  set --
  source "$task_dir/functions.sh"
  trap - EXIT
  INSTALL_DIR="$task_dir/atomic-bin"
  mkdir -p "$INSTALL_DIR" "$task_dir/outside"
  printf 'new binary\n' > "$task_dir/source"
  printf 'untouched\n' > "$task_dir/outside/omg"
  ln -s "$task_dir/outside" "$INSTALL_DIR/omg"
  install_binary "$task_dir/source" "$INSTALL_DIR/omg"
  [[ ! -L "$INSTALL_DIR/omg" && -f "$INSTALL_DIR/omg" ]]
  [[ $(cat "$task_dir/outside/omg") == untouched ]]
  printf 'updated binary\n' > "$task_dir/source"
  install_binary "$task_dir/source" "$INSTALL_DIR/omg"
  cmp "$task_dir/source" "$INSTALL_DIR/omg"
  mkdir "$INSTALL_DIR/omgd"
  if (install_binary "$task_dir/source" "$INSTALL_DIR/omgd"); then
    printf 'Installer accepted a directory destination\n' >&2
    exit 1
  fi
  [[ ! -e "$INSTALL_DIR/omgd/source" && ! -e "$INSTALL_DIR/omgd/omgd" ]]
)
# Piped definitions run in an attacker-controlled checkout, even with the old
# auto-detection bait. They must never qualify as an explicit source install.
mkdir -p "$task_dir/ambient"
printf '[package]\nname = "omg"\n' > "$task_dir/ambient/Cargo.toml"
{ cat "$task_dir/functions.sh"; printf '\n[[ "$IS_SOURCE_INSTALL" == false ]]\n'; } | (cd "$task_dir/ambient"; bash)
printf 'Installer security scenarios passed\n'
