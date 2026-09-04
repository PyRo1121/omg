#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == --help ]]; then
  printf 'Usage: scripts/benchmark-qemu-ubuntu.sh\nRuns the pinned Ubuntu guest and OMG v0.1.218 candidate-info benchmark locally.\nRequires Docker access, KVM, gh, and coreutils. No compilation.\nOne guest, two vCPUs, 1536 MiB guest RAM, 3 GiB controller memory limit.\nEvidence is retained under ~/.cache/build-targets/omg-qemu-benchmark/.\n'
  exit 0
fi
[[ $# == 0 ]] || { printf 'error: unexpected arguments\n' >&2; exit 2; }
for tool in docker gh timeout sha256sum jq; do
  command -v "$tool" >/dev/null || { printf 'error: missing %s\n' "$tool" >&2; exit 3; }
done
[[ -r /dev/kvm && -w /dev/kvm ]] || { printf 'error: KVM is unavailable\n' >&2; exit 3; }
timeout 15 docker info >/dev/null
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="$HOME/.cache/build-targets/omg-qemu-benchmark"
mkdir -p "$root"
work="$(mktemp -d "$root/run-XXXXXX")"
controller="omg-qemu-${work##*/}"
printf 'Evidence: %s\n' "$work"

cleanup() {
  local rc=$? remaining
  trap - EXIT
  if [[ ${started:-false} == true ]]; then
    timeout --kill-after=5s 15s docker exec "$controller" sh -c 'if test -f /work/ubuntu/qemu.pid; then kill "$(cat /work/ubuntu/qemu.pid)" 2>/dev/null || true; fi; rm -f /work/ubuntu/client-key /work/ubuntu/guest-host-key /work/ubuntu/user-data /work/ubuntu/seed.img /work/ubuntu/overlay.qcow2 /work/ubuntu/noble-server-cloudimg-amd64.img' >> "$work/cleanup.log" 2>&1 || rc=3
    timeout --kill-after=5s 15s docker rm --force "$controller" >> "$work/cleanup.log" 2>&1 || rc=3
    if remaining="$(timeout 15 docker ps --all --quiet --filter "name=^/${controller}$")" && [[ -z "$remaining" ]]; then
      printf 'verified absent: %s\n' "$controller" >> "$work/cleanup.log"
    else
      rc=3
    fi
  fi
  printf 'exit_code=%s\n' "$rc" > "$work/exit-status.txt"
  jq -n --argjson rc "$rc" --argjson elapsed "$SECONDS" \
    '[{case_id:"qemu-ubuntu-info",distro:"ubuntu",result:(if $rc == 0 then "PASS" else "HARNESS_ERROR" end),exit_code:$rc,elapsed_seconds:$elapsed}]' > "$work/results.json"
  if ! timeout --kill-after=2s 12s env OMG_SMOKE_RELEASE=v0.1.218 \
      "$repo_root/scripts/report-smoke-sentry.sh" "$work/results.json" > "$work/reporting.log" 2>&1; then
    printf 'warning: Sentry reporting failed; local evidence is intact\n' >&2
  fi
  exit "$rc"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
{
  date -u
  uname -a
  cat /proc/loadavg
  grep -E 'MemTotal|MemAvailable|SwapFree' /proc/meminfo
  ps -eo pid,comm,pcpu --sort=-pcpu | head -15
} > "$work/host-metadata.txt"
mkdir -p "$work/release"
archive=omg-v0.1.218-x86_64-linux-ubuntu.tar.gz
timeout 120 gh release download v0.1.218 --repo PyRo1121/omg --pattern "$archive" --dir "$work/release"
printf '82c4765c57ac46936422b411c9f5805bd3cd3092ca60488534cae510751eb73a  %s\n' "$work/release/$archive" | sha256sum -c - > "$work/release-checksum.txt"
started=true
timeout 120 docker run -d --name "$controller" --cpus 2 --memory 3g --memory-swap 3g --device /dev/kvm \
  --mount "type=bind,src=$work,dst=/work" --workdir /work \
  debian:bookworm@sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931 sleep infinity > "$work/controller-id.txt"
timeout 300 docker exec "$controller" sh -c 'apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends qemu-system-x86 qemu-utils cloud-image-utils openssh-client curl ca-certificates' > "$work/controller-setup.log" 2>&1

timeout 300 docker exec "$controller" sh -c 'set -e; mkdir -p ubuntu; cd ubuntu; curl --fail --location --max-time 240 -o noble-server-cloudimg-amd64.img https://cloud-images.ubuntu.com/noble/20260826/noble-server-cloudimg-amd64.img; echo "d0fe84bb5f80853425fa6be28e2c106f30104c3cfe8611933f2e65c9b63f0e30  noble-server-cloudimg-amd64.img" | sha256sum -c -; qemu-system-x86_64 --version; qemu-img info noble-server-cloudimg-amd64.img' > "$work/image-setup.log" 2>&1

cat > "$work/boot.sh" <<'BOOT'
#!/usr/bin/env bash
set -euo pipefail
cd /work/ubuntu
ssh-keygen -q -t ed25519 -N '' -f client-key
ssh-keygen -q -t ed25519 -N '' -f guest-host-key
{
  printf '#cloud-config\nusers:\n  - name: bench\n    groups: [sudo]\n    sudo: "ALL=(ALL) NOPASSWD:ALL"\n    shell: /bin/bash\n    ssh_authorized_keys:\n      - '
  cat client-key.pub
  printf 'ssh_pwauth: false\ndisable_root: true\nssh_keys:\n  ed25519_private: |\n'
  sed 's/^/    /' guest-host-key
  printf '  ed25519_public: '
  cat guest-host-key.pub
} > user-data
chmod 600 user-data
printf 'instance-id: omg-benchmark-fresh\nlocal-hostname: omg-bench\n' > meta-data
printf '[127.0.0.1]:2222 ' > known_hosts
cat guest-host-key.pub >> known_hosts
cloud-localds seed.img user-data meta-data
qemu-img create -f qcow2 -F qcow2 -b /work/ubuntu/noble-server-cloudimg-amd64.img overlay.qcow2
qemu-img resize overlay.qcow2 12G
qemu-system-x86_64 -machine q35,accel=kvm -cpu host -smp 2 -m 1536 \
  -display none -serial file:serial.log -qmp unix:qmp.sock,server=on,wait=off \
  -drive file=overlay.qcow2,if=virtio,format=qcow2 \
  -drive file=seed.img,if=virtio,format=raw \
  -netdev user,id=n,hostfwd=tcp:127.0.0.1:2222-:22 -device virtio-net-pci,netdev=n \
  -daemonize -pidfile qemu.pid
opts=(-i client-key -p 2222 -o BatchMode=yes -o ConnectTimeout=2 -o StrictHostKeyChecking=yes -o UserKnownHostsFile=known_hosts)
for attempt in {1..90}; do
  kill -0 "$(<qemu.pid)"
  if ssh "${opts[@]}" bench@127.0.0.1 true 2>/dev/null; then
    timeout 180 ssh "${opts[@]}" bench@127.0.0.1 'cloud-init status --wait; cat /etc/os-release; uname -r; sudo -n true'
    exit 0
  fi
  sleep 2
done
exit 1
BOOT
timeout 400 docker exec "$controller" bash /work/boot.sh > "$work/boot.log" 2>&1

cat > "$work/guest-benchmark.sh" <<'GUEST'
#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C NO_COLOR=1
cd "$HOME"
echo '82c4765c57ac46936422b411c9f5805bd3cd3092ca60488534cae510751eb73a  release.tar.gz' | sha256sum -c -
tar -xzf release.tar.gz
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends hyperfine
mkdir -p evidence
bin="$HOME/omg-v0.1.218-x86_64-linux-ubuntu/omg"
"$bin" info tree > evidence/omg-info.txt
apt-cache --no-all-versions show tree > evidence/apt-cache-info.txt
apt show tree > evidence/apt-info.txt 2> evidence/apt-info.stderr
version="$(awk '/^Version: / { print $2; exit }' evidence/apt-cache-info.txt)"
[[ -n "$version" ]]
grep -Eq '^[[:space:]]*Name: tree$' evidence/omg-info.txt
grep -Fq "Version: $version" evidence/omg-info.txt
grep -Fxq 'Package: tree' evidence/apt-info.txt
grep -Fxq "Version: $version" evidence/apt-info.txt
{
  date -u
  cat /etc/os-release
  uname -a
  hyperfine --version
  apt --version
  sha256sum "$bin"
  printf 'scenario=candidate-package-info\npackage=tree\nversion=%s\n' "$version"
  printf 'cache=warm; output=pipe; fresh-process-per-sample; no explicit daemon startup\n'
  printf 'equivalence=name and candidate version; native output includes additional metadata\n'
  cat /proc/meminfo
  cat /proc/loadavg
} > evidence/guest-metadata.txt
find /var/lib/apt/lists -maxdepth 1 -type f ! -name lock -exec sha256sum {} + > evidence/repository-hashes.txt
hyperfine --shell=none --output=pipe --warmup 3 --runs 30 \
  --export-json evidence/info.json --export-markdown evidence/info.md \
  --command-name 'OMG info tree' "$bin info tree" \
  --command-name 'apt-cache candidate info' 'apt-cache --no-all-versions show tree' \
  --command-name 'apt show tree' 'apt show tree'
GUEST
ssh_opts=(-i client-key -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=known_hosts)
timeout 60 docker exec -w /work/ubuntu "$controller" scp "${ssh_opts[@]}" -P 2222 /work/release/"$archive" bench@127.0.0.1:release.tar.gz
timeout 60 docker exec -w /work/ubuntu "$controller" scp "${ssh_opts[@]}" -P 2222 /work/guest-benchmark.sh bench@127.0.0.1:guest-benchmark.sh
timeout 300 docker exec -w /work/ubuntu "$controller" ssh "${ssh_opts[@]}" -p 2222 bench@127.0.0.1 'bash guest-benchmark.sh' > "$work/benchmark.log" 2>&1
timeout 60 docker exec -w /work/ubuntu "$controller" scp -r "${ssh_opts[@]}" -P 2222 bench@127.0.0.1:evidence /work/ubuntu/
printf 'Completed. Raw samples: %s/ubuntu/evidence/info.json\n' "$work"
