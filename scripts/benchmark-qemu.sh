#!/usr/bin/env bash
set -euo pipefail

distro=all
tag=v0.1.218
staged_dir=
benchmark=false
root="$HOME/.cache/build-targets/omg-qemu-benchmark"
while (($#)); do
  case "$1" in
    --distro|--release|--staged-dir|--evidence-dir)
      [[ $# -ge 2 && -n "$2" ]] || exit 2
      case "$1" in
        --distro) distro=$2 ;; --release) tag=$2 ;; --staged-dir) staged_dir=$2 ;; --evidence-dir) root=$2 ;;
      esac
      shift 2 ;;
    --benchmark) benchmark=true; shift ;;
    --help) printf 'Usage: scripts/benchmark-qemu.sh [--distro all|arch|debian|ubuntu|fedora] [--release vVERSION] [--staged-dir DIR] [--evidence-dir DIR] [--benchmark]\nRuns sequential disposable KVM guests with pinned images, reboot, sudo, package lifecycle and optional warm info timing. Requires Docker, KVM, gh, jq, coreutils. No compilation or host package changes.\n'; exit 0 ;;
    *) printf 'error: unknown argument %s\n' "$1" >&2; exit 2 ;;
  esac
done
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 2
case "$distro" in all|arch|debian|ubuntu|fedora) ;; *) exit 2 ;; esac
[[ -z "$staged_dir" || -d "$staged_dir" ]] || exit 2
command -v jq >/dev/null || exit 3
source_kind=published
[[ -z "$staged_dir" ]] || source_kind=staged
mkdir -p "$root"
root=$(cd "$root" && pwd)
if [[ "$distro" == all ]]; then
  suite=$(mktemp -d "$root/suite-XXXXXX")
  rc=0
  args=(--release "$tag")
  [[ -z "$staged_dir" ]] || args+=(--staged-dir "$staged_dir")
  [[ "$benchmark" == false ]] || args+=(--benchmark)
  jq -n --arg source "$source_kind" '["arch", "debian", "ubuntu", "fedora"] | map({case_id:("qemu-"+.+"-lifecycle"), distro:., result:"NOT_RUN", artifact_source:$source, exit_code:null, elapsed_seconds:0})' > "$suite/results.json"
  for target in arch debian ubuntu fedora; do
    jq --arg target "$target" 'map(if .distro == $target then .result = "INCOMPLETE" else . end)' "$suite/results.json" > "$suite/results.next.json"
    mv "$suite/results.next.json" "$suite/results.json"
    "$0" --distro "$target" --evidence-dir "$suite/$target" "${args[@]}" || rc=1
    reports=("$suite/$target"/run-*/results.json)
    if [[ ${#reports[@]} -eq 1 && -f "${reports[0]}" ]] && jq -e --arg target "$target" 'length == 1 and .[0].distro == $target' "${reports[0]}" >/dev/null; then
      jq --arg target "$target" --slurpfile report "${reports[0]}" 'map(if .distro == $target then $report[0][0] else . end)' "$suite/results.json" > "$suite/results.next.json"
      mv "$suite/results.next.json" "$suite/results.json"
    else rc=1; fi
  done
  printf 'Suite evidence: %s\n' "$suite"
  exit "$rc"
fi
hash_tool=sha256sum
firmware=bios
ssh_service=sshd
case "$distro" in
  arch)
    image_url=https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg-20260901.583572.qcow2
    image_hash=e3e688f97a71b265ce202905a504253f60f3680cf57d011a45411c43bedfa930
    firmware=uefi ;;
  debian)
    image_url=https://cloud.debian.org/images/cloud/bookworm/20260903-2590/debian-12-generic-amd64-20260903-2590.qcow2
    image_hash=804377dd07318360c39a75e57b326243442a43bae1e12b33d5f490a64713c15c080a0323cb55d52b381139db9187702d38f36b8b142b8ef36da1031d9de41c2d
    hash_tool=sha512sum
    ssh_service=ssh ;;
  ubuntu)
    image_url=https://cloud-images.ubuntu.com/noble/20260826/noble-server-cloudimg-amd64.img
    image_hash=d0fe84bb5f80853425fa6be28e2c106f30104c3cfe8611933f2e65c9b63f0e30
    ssh_service=ssh ;;
  fedora)
    image_url=https://download.fedoraproject.org/pub/fedora/linux/releases/44/Cloud/x86_64/images/Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2
    image_hash=28680fe5b371a5a82ebf43a31926e086a168e59949d03969c5093e7071f90b7f ;;
esac
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d "$root/run-XXXXXX")
controller="omg-qemu-${work##*/}"
printf 'Starting %s. Evidence: %s\n' "$distro" "$work"
result=HARNESS_ERROR
cleanup() {
  local rc=$? remaining
  trap - EXIT
  if [[ ${started:-false} == true ]]; then
    timeout --kill-after=5s 60s docker rm --force "$controller" >> "$work/cleanup.log" 2>&1 || { rc=3; result=HARNESS_ERROR; }
    if remaining=$(timeout 15 docker ps -aq --filter "name=^/${controller}$") && [[ -z "$remaining" ]]; then
      printf 'verified absent: %s\n' "$controller" >> "$work/cleanup.log"
    else rc=3; result=HARNESS_ERROR; fi
  fi
  rm -f "$work/guest"/{client-key,guest-host-key,user-data,seed.img,overlay.qcow2,base.qcow2,vars.fd,qemu.pid} || { rc=3; result=HARNESS_ERROR; }
  for file in client-key guest-host-key user-data seed.img overlay.qcow2 base.qcow2 vars.fd qemu.pid; do
    if [[ -e "$work/guest/$file" ]]; then rc=3; result=HARNESS_ERROR; fi
  done
  if [[ "$rc" -ne 0 && "$result" == PASS ]]; then result=HARNESS_ERROR; fi
  jq -n --arg distro "$distro" --arg result "$result" --arg source "$source_kind" --argjson rc "$rc" --argjson elapsed "$SECONDS" \
    '[{case_id:("qemu-"+$distro+"-lifecycle"),distro:$distro,result:$result,artifact_source:$source,exit_code:$rc,elapsed_seconds:$elapsed}]' > "$work/results.json"
  timeout --kill-after=2s 12s env OMG_SMOKE_RELEASE="$tag" "$repo_root/scripts/report-smoke-sentry.sh" "$work/results.json" > "$work/reporting.log" 2>&1 || true
  printf '%s %s. Evidence: %s\n' "$distro" "$result" "$work"
  exit "$rc"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$work/release" "$work/guest"
: > "$work/guest/serial.log"
for tool in docker timeout sha256sum; do command -v "$tool" >/dev/null || exit 3; done
[[ -n "$staged_dir" ]] || { command -v gh >/dev/null || exit 3; }
timeout --kill-after=2s 15s docker version --format '{{.Server.Version}}' > "$work/engine-preflight.log" 2>&1 || exit 3
{ date -u; uname -a; cat /proc/loadavg; grep -E 'MemTotal|MemAvailable|SwapFree' /proc/meminfo; } > "$work/host-metadata.txt"
archive="omg-${tag}-x86_64-linux-${distro}.tar.gz"
if [[ -n "$staged_dir" ]]; then
  cp "$staged_dir/$archive" "$staged_dir/$archive.sha256" "$work/release/"
else
  timeout 120 gh release download "$tag" --repo PyRo1121/omg --pattern "$archive" --pattern "$archive.sha256" --dir "$work/release"
fi
read -r digest filename extra < "$work/release/$archive.sha256"
[[ "$digest" =~ ^[0-9a-f]{64}$ && "$filename" == "$archive" && -z "${extra:-}" ]]
[[ $(wc -l < "$work/release/$archive.sha256") -eq 1 ]]
(cd "$work/release" && sha256sum -c "$archive.sha256") > "$work/release-checksum.txt"
printf 'distro=%s\nrelease=%s\nartifact_source=%s\nimage_url=%s\nimage_digest=%s\nfirmware=%s\n' "$distro" "$tag" "$source_kind" "$image_url" "$image_hash" "$firmware" > "$work/metadata.txt"
started=true
timeout 120 docker run -d --name "$controller" --cpus 2 --memory 3g --memory-swap 3g --device /dev/kvm \
  --mount "type=bind,src=$work,dst=/work" --workdir /work \
  debian:bookworm@sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931 sleep infinity > "$work/controller-id.txt"
timeout 300 docker exec "$controller" sh -c 'apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends qemu-system-x86 qemu-utils cloud-image-utils openssh-client curl ca-certificates ovmf' > "$work/controller-setup.log" 2>&1
timeout 360 docker exec "$controller" bash -c 'set -e; cd /work/guest; curl --fail --location --max-time 300 -o base.qcow2 "$1"; printf "%s  base.qcow2\n" "$2" | "$3" -c -; qemu-system-x86_64 --version; qemu-img info base.qcow2' _ "$image_url" "$image_hash" "$hash_tool" > "$work/image-setup.log" 2>&1
cat > "$work/boot.sh" <<'BOOT'
#!/usr/bin/env bash
set -euo pipefail
cd /work/guest
ssh-keygen -q -t ed25519 -N '' -f client-key
ssh-keygen -q -t ed25519 -N '' -f guest-host-key
{
  printf '#cloud-config\nusers:\n  - name: bench\n    sudo: "ALL=(ALL) NOPASSWD:ALL"\n    shell: /bin/bash\n    ssh_authorized_keys:\n      - '
  cat client-key.pub
  printf 'ssh_pwauth: false\ndisable_root: true\nssh_keys:\n  ed25519_private: |\n'
  sed 's/^/    /' guest-host-key
  printf '  ed25519_public: '
  cat guest-host-key.pub
} > user-data
chmod 600 user-data
printf 'instance-id: omg-qemu-fresh\nlocal-hostname: omg-qa\n' > meta-data
printf '[127.0.0.1]:2222 ' > known_hosts
cat guest-host-key.pub >> known_hosts
cloud-localds seed.img user-data meta-data
qemu-img create -f qcow2 -F qcow2 -b /work/guest/base.qcow2 overlay.qcow2
qemu-img resize overlay.qcow2 12G
firmware=()
if [[ "$1" == uefi ]]; then
  cp /usr/share/OVMF/OVMF_VARS_4M.fd vars.fd
  firmware=(-drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd -drive if=pflash,format=raw,file=vars.fd)
fi
qemu-system-x86_64 -machine q35,accel=kvm -cpu host -smp 2 -m 1536 \
  "${firmware[@]}" -display none -serial file:serial.log \
  -drive file=overlay.qcow2,if=virtio,format=qcow2 -drive file=seed.img,if=virtio,format=raw \
  -netdev user,id=n,ipv6=off,hostfwd=tcp:127.0.0.1:2222-:22 -device virtio-net-pci,netdev=n \
  -daemonize -pidfile qemu.pid
opts=(-i client-key -p 2222 -o BatchMode=yes -o ConnectTimeout=2 -o ServerAliveInterval=5 -o ServerAliveCountMax=3 -o StrictHostKeyChecking=yes -o UserKnownHostsFile=known_hosts)
wait_ssh() {
  for attempt in {1..120}; do
    kill -0 "$(<qemu.pid)"
    if ssh "${opts[@]}" bench@127.0.0.1 true 2>/dev/null; then return 0; fi
    sleep 2
  done
  return 1
}
wait_ssh
timeout 180 ssh "${opts[@]}" bench@127.0.0.1 'cloud-init status --wait; cat /etc/os-release; uname -r; sudo -n true'
ssh "${opts[@]}" bench@127.0.0.1 "sudo -n systemctl enable '$2'"
before=$(ssh "${opts[@]}" bench@127.0.0.1 cat /proc/sys/kernel/random/boot_id)
ssh "${opts[@]}" bench@127.0.0.1 'sudo -n systemctl reboot' || true
for attempt in {1..120}; do
  if after=$(ssh "${opts[@]}" bench@127.0.0.1 cat /proc/sys/kernel/random/boot_id 2>/dev/null) && [[ "$after" != "$before" ]]; then
    printf 'reboot verified: %s -> %s\n' "$before" "$after"
    ssh "${opts[@]}" bench@127.0.0.1 "sudo -n true; systemctl is-active '$2'"
    exit 0
  fi
  sleep 2
done
exit 1
BOOT
timeout 700 docker exec "$controller" bash /work/boot.sh "$firmware" "$ssh_service" > "$work/boot.log" 2>&1
cat > "$work/guest-check.sh" <<'GUEST'
#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C NO_COLOR=1
cd "$HOME"
distro=$1; tag=$2; digest=$3; benchmark=$4
actual_id=$(awk -F= '$1 == "ID" {gsub(/"/, "", $2); print $2}' /etc/os-release)
[[ "$actual_id" == "$distro" && $(uname -m) == x86_64 ]] || exit 120
mkdir -p evidence
trap 'status=$?; printf "%s\n" "$status" > evidence/exit-code' EXIT
printf '%s  release.tar.gz\n' "$digest" | sha256sum -c -
tar -xzf release.tar.gz
bin="$HOME/omg-${tag}-x86_64-linux-${distro}/omg"
[[ $("$bin" --version | head -1 | tr -d '[:space:]') == "omg${tag#v}" ]]
case "$distro" in
  arch) sudo -n pacman -Syu --noconfirm >/dev/null || exit 120; native=(pacman -Qi tree); version_cmd=(pacman -Q tree) ;;
  debian|ubuntu)
    sudo -n systemctl stop apt-daily.timer apt-daily-upgrade.timer
    if [[ "$distro" == ubuntu ]]; then
      sudo -n sed -i 's|http://archive.ubuntu.com/ubuntu|https://archive.ubuntu.com/ubuntu|g; s|http://security.ubuntu.com/ubuntu|https://security.ubuntu.com/ubuntu|g' /etc/apt/sources.list.d/ubuntu.sources
    fi
    printf 'Acquire::ForceIPv4 "true";\nAcquire::Retries "2";\nAcquire::http::Timeout "30";\nAcquire::https::Timeout "30";\nAcquire::Languages "none";\nAcquire::IndexTargets::deb::DEP-11::DefaultEnabled "false";\nAcquire::IndexTargets::deb::CNF::DefaultEnabled "false";\n' | sudo -n tee /etc/apt/apt.conf.d/99omg-qa-network > evidence/apt-network.conf
    sudo -n apt-get update > evidence/index-update.txt 2>&1 || exit 120
    native=(apt-cache --no-all-versions show tree)
    version_cmd=(dpkg-query -W '-f=${Version}\n' tree) ;;
  fedora) sudo -n dnf -y makecache >/dev/null || exit 120; native=(rpm -qi tree); version_cmd=(rpm -q --qf '%{VERSION}-%{RELEASE}\n' tree) ;;
esac
installed() {
  case "$distro" in arch) pacman -Q tree ;; debian|ubuntu) [[ $(dpkg-query -W '-f=${Status}' tree 2>/dev/null) == 'install ok installed' ]] ;; fedora) rpm -q tree ;; esac
}
if installed >/dev/null 2>&1; then echo 'fixture requires tree absent' >&2; exit 120; fi
"$bin" search tree > evidence/search.txt
grep -Eqi '^[[:space:]]+tree[[:space:]]' evidence/search.txt
sudo -n "$bin" install --yes tree
installed
"$bin" info tree > evidence/omg-info.txt
"${native[@]}" > evidence/native-info.txt
version=$("${version_cmd[@]}")
[[ "$distro" != arch ]] || version=${version#tree }
[[ $(awk '$1 == "Name:" {print $2}' evidence/omg-info.txt) == tree ]]
[[ $(awk '$1 == "Version:" {print $2}' evidence/omg-info.txt) == "$version" ]]
if [[ "$benchmark" == true ]]; then
  case "$distro" in
    arch) sudo -n pacman -S --noconfirm --needed hyperfine || exit 120 ;;
    debian|ubuntu) sudo -n env DEBIAN_FRONTEND=noninteractive apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 install -y --no-install-recommends hyperfine || exit 120 ;;
    fedora) sudo -n dnf install -y hyperfine || exit 120 ;;
  esac
  hyperfine --shell=none --output=pipe --warmup 3 --runs 30 --export-json evidence/info.json \
    --command-name 'OMG installed info' "$bin info tree" --command-name 'Native info' "${native[*]}"
fi
sudo -n "$bin" remove --yes tree
if installed >/dev/null 2>&1; then echo 'package remains installed' >&2; exit 1; fi
if [[ "$distro" == debian || "$distro" == ubuntu ]]; then
  apt-get download tree || exit 120
  packages=("$HOME"/tree_*.deb)
  [[ ${#packages[@]} -eq 1 && -f "${packages[0]}" ]] || exit 120
  sha256sum "${packages[0]}" > evidence/local-package.sha256
  if "$bin" install --yes "${packages[0]}" > evidence/local-consent.txt 2>&1; then
    echo 'local archive was accepted without consent' >&2; exit 1
  fi
  grep -Fq 'require explicit consent' evidence/local-consent.txt
  sudo -n "$bin" install --allow-local-file --yes "${packages[0]}"
  installed
  sudo -n "$bin" remove --yes tree
  if installed >/dev/null 2>&1; then echo 'local package remains installed' >&2; exit 1; fi
fi
case "$distro" in
  arch) pacman -Q > evidence/installed-after.txt; sha256sum /var/lib/pacman/sync/*.db > evidence/repository-hashes.txt ;;
  debian|ubuntu) dpkg-query -W > evidence/installed-after.txt; find /var/lib/apt/lists -maxdepth 1 -type f ! -name lock -exec sha256sum {} + > evidence/repository-hashes.txt ;;
  fedora) rpm -qa > evidence/installed-after.txt; find /var/cache/libdnf5 -type f -name repomd.xml -exec sha256sum {} + > evidence/repository-hashes.txt ;;
esac
{ cat /etc/os-release; uname -a; sha256sum "$bin"; printf 'native_version=%s\n' "$version"; } > evidence/guest-metadata.txt
echo 'PASS: package lifecycle and native version parity'
GUEST
opts=(-i client-key -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=known_hosts)
timeout 60 docker exec -w /work/guest "$controller" scp "${opts[@]}" -P 2222 "/work/release/$archive" bench@127.0.0.1:release.tar.gz
timeout 60 docker exec -w /work/guest "$controller" scp "${opts[@]}" -P 2222 /work/guest-check.sh bench@127.0.0.1:guest-check.sh
rc=0
timeout 600 docker exec -w /work/guest "$controller" ssh "${opts[@]}" -p 2222 bench@127.0.0.1 "bash guest-check.sh '$distro' '$tag' '$digest' '$benchmark'" > "$work/guest-check.log" 2>&1 || rc=$?
timeout 60 docker exec -w /work/guest "$controller" scp -r "${opts[@]}" -P 2222 bench@127.0.0.1:evidence /work/guest/ > "$work/evidence-copy.log" 2>&1
guest_rc=$(<"$work/guest/evidence/exit-code")
if [[ ! "$guest_rc" =~ ^[0-9]+$ || "$guest_rc" != "$rc" ]]; then
  printf 'Guest exit %s differs from transport exit %s\n' "$guest_rc" "$rc" >&2
  exit 3
fi
case "$rc" in 0) result=PASS ;; 120|124|125|126|127|137|255) result=HARNESS_ERROR ;; *) result=PRODUCT_FAIL ;; esac
exit "$rc"
