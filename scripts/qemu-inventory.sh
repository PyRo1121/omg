#!/usr/bin/env bash
# qemu-inventory.sh — drive tests/cli_behavior_inventory.tsv rows inside a
# running QEMU guest over SSH and record machine-readable evidence.
#
# The guest must already be up (see scripts/benchmark-qemu.sh, which calls
# this when --inventory-tiers is set). Rows run in file order so the
# TSV `requires` DAG is satisfied naturally. Disposable guests make
# package/service-mutation rows safe, but they still need --allow-mutations.
set -euo pipefail

work=""; distro=""; tiers=""; tag=""; binary=""; tsv=""
allow_mutations=false
allow_credentialed=false
row_timeout=120
ssh_port=2222
ssh_user=bench
while (($#)); do
  case "$1" in
    --work|--distro|--tiers|--tag|--binary|--tsv|--row-timeout|--ssh-port|--ssh-user)
      [[ $# -ge 2 && -n "$2" ]] || exit 2
      case "$1" in
        --work) work=$2 ;; --distro) distro=$2 ;; --tiers) tiers=$2 ;;
        --tag) tag=$2 ;; --binary) binary=$2 ;; --tsv) tsv=$2 ;;
        --row-timeout) row_timeout=$2 ;; --ssh-port) ssh_port=$2 ;; --ssh-user) ssh_user=$2 ;;
      esac
      shift 2 ;;
    --allow-mutations) allow_mutations=true; shift ;;
    --allow-credentialed) allow_credentialed=true; shift ;;
    --help) printf 'Usage: qemu-inventory.sh --work DIR --distro D --tiers CSV --tag TAG --binary GUEST_PATH --tsv FILE [--allow-mutations] [--allow-credentialed] [--row-timeout S]\nRuns TSV-selected rows in the guest at 127.0.0.1 and writes evidence.\n'; exit 0 ;;
    *) printf 'error: unknown argument %s\n' "$1" >&2; exit 2 ;;
  esac
done
[[ -n "$work" && -n "$distro" && -n "$tiers" && -n "$tag" && -n "$binary" && -n "$tsv" ]] || exit 2
[[ "$row_timeout" =~ ^[0-9]+$ && "$row_timeout" -gt 0 ]] || exit 2
case "$distro" in arch|debian|ubuntu|fedora) ;; *) exit 2 ;; esac
command -v ssh jq >/dev/null || exit 3
[[ -f "$tsv" ]] || exit 2

root=$(cd "$work" && pwd)
guest="$root/guest"
out="$root/inventory"
mkdir -p "$out/rows"
# -n keeps ssh from forwarding (and draining) this loop's stdin, which is the
# TSV stream: without it only the first tier-matching row ever executes.
opts=(-n -i "$guest/client-key" -p "$ssh_port" -o BatchMode=yes -o ConnectTimeout=5
  -o ServerAliveInterval=5 -o ServerAliveCountMax=3 -o StrictHostKeyChecking=yes
  -o UserKnownHostsFile="$guest/known_hosts")
target="$ssh_user@127.0.0.1"

# Selected tiers as comma-wrappedneedle, same idiom as release-smoke.sh.
want=",$tiers,"
summary_tmp="$out/results.json.tmp"
printf '[]' > "$summary_tmp"
pass=0; fail=0; skipped=0
record() { # case_id result exit_code elapsed
  local entry
  entry=$(jq -n --arg c "$1" --arg d "$distro" --arg r "$2" --argjson e "$3" --argjson s "$4" \
    '{case_id:$c, distro:$d, result:$r, artifact_source:"inventory", exit_code:$e, elapsed_seconds:$s}')
  jq --slurpfile e <(printf '%s' "$entry") '. + [$e[0]]' "$summary_tmp" > "$summary_tmp.next"
  mv "$summary_tmp.next" "$summary_tmp"
}

# Process substitution (not a pipeline): pass/fail counters below must
# survive the loop; a `tail | while` pipeline would trap them in a subshell.
while IFS=$'\t' read -r case args_json safety expected_exit expected_ux requires tier targets assertions cleanup; do
  # Case ids flow into a remote shell command below: reject anything
  # outside the identifier shape instead of executing it.
  if [[ ! "$case" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
    printf 'case=%s invalid identifier\n' "$case"
    # Sanitized id: one bad TSV row must fail loudly without poisoning
    # the whole results file for the downstream schema gate.
    safe="invalid-$(printf '%s' "$case" | tr -c 'a-z0-9-' '-' | cut -c1-80)"
    record "qemu-$distro-$safe" FAIL -1 0; fail=$((fail+1)); continue
  fi
  # Tier filter (tiers are comma-separated in the TSV too).
  hit=false
  IFS=',' read -ra tier_list <<< "$tier"
  for t in "${tier_list[@]}"; do
    if [[ "$want" == *",$t,"* ]]; then hit=true; break; fi
  done
  if [[ "$hit" == false ]]; then continue; fi
  # Declaration-only rows are parse-level, covered by cli_comprehensive.
  if [[ "$expected_ux" == declared ]]; then
    record "qemu-$distro-$case" SKIPPED -1 0; skipped=$((skipped+1)); continue
  fi
  # Per-distro target status.
  status=""
  if [[ "$targets" == hermetic:pass ]]; then status=pass
  else
    IFS=',' read -ra target_list <<< "$targets"
    for t in "${target_list[@]}"; do
      if [[ "$t" == "$distro:"* ]]; then status="${t#*:}"; break; fi
    done
  fi
  if [[ -z "$status" || "$status" == pending ]]; then
    record "qemu-$distro-$case" SKIPPED -1 0; skipped=$((skipped+1)); continue
  fi
  allow_fail=false
  if [[ "$status" == known-defect ]]; then allow_fail=true; fi
  # Credentialed rows need a real token in the guest: never run them
  # without an explicit opt-in (tiers match by substring, so "network"
  # would otherwise select "network,credentialed" rows).
  if [[ ",$tier," == *",credentialed,"* && "$allow_credentialed" == false ]]; then
    record "qemu-$distro-$case" SKIPPED -1 0; skipped=$((skipped+1)); continue
  fi
  # Safety gate.
  case "$safety" in
    interactive) record "qemu-$distro-$case" SKIPPED -1 0; skipped=$((skipped+1)); continue ;;
    package-mutation|service-mutation)
      if [[ "$allow_mutations" == false ]]; then
        record "qemu-$distro-$case" SKIPPED -1 0; skipped=$((skipped+1)); continue
      fi ;;
  esac
  # Build the remote command: binary + per-element @sh quoting.
  arg_string=$(printf '%s' "$args_json" | jq -r '[.[] | @sh] | join(" ")')
  rowdir="\$HOME/inventory-work/$case"
  remote="rm -rf $rowdir && mkdir -p $rowdir && cd $rowdir && NO_COLOR=1 LC_ALL=C $binary $arg_string"
  start=$SECONDS
  # shellcheck disable=SC2086
  if timeout "$row_timeout" ssh "${opts[@]}" "$target" "$remote" > "$out/rows/$case.log" 2>&1; then
    rc=0
  else
    rc=$?
  fi
  elapsed=$((SECONDS - start))
  verdict=PASS
  if [[ "$expected_exit" =~ ^[0-9]+$ && "$rc" != "$expected_exit" ]]; then verdict=FAIL; fi
  if [[ "$verdict" == FAIL && "$allow_fail" == true ]]; then verdict=PASS; fi
  record "qemu-$distro-$case" "$verdict" "$rc" "$elapsed"
  if [[ "$verdict" == PASS ]]; then pass=$((pass+1)); else fail=$((fail+1)); fi
  printf 'case=%s exit=%s verdict=%s\n' "$case" "$rc" "$verdict"
done < <(tail -n +2 "$tsv")
mv "$summary_tmp" "$out/results.json"
printf '{"pass":%s,"fail":%s,"skipped":%s}\n' "$pass" "$fail" "$skipped" > "$out/summary.json"
cat "$out/summary.json"
[[ "$fail" -eq 0 ]]
