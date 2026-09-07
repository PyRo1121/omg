#!/usr/bin/env bash
set -euo pipefail

distro=all
tag=v0.1.218
staged_dir=
arch=
print_pins=false
benchmark=false
inventory_tiers=
inventory_mutations=false
root="$HOME/.cache/build-targets/omg-qemu-benchmark"
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
while (($#)); do
  case "$1" in
    --distro|--release|--staged-dir|--evidence-dir|--inventory-tiers|--arch)
      [[ $# -ge 2 && -n "$2" ]] || exit 2
      case "$1" in
        --distro) distro=$2 ;; --release) tag=$2 ;; --staged-dir) staged_dir=$2 ;; --evidence-dir) root=$2 ;;
        --inventory-tiers) inventory_tiers=$2 ;; --arch) arch=$2 ;;
      esac
      shift 2 ;;
    --benchmark) benchmark=true; shift ;;
    --print-pins) print_pins=true; shift ;;
    --inventory-allow-mutations) inventory_mutations=true; shift ;;
    --help) printf 'Usage: scripts/benchmark-qemu.sh [--distro all|arch|debian|ubuntu|fedora] [--arch x86_64|aarch64] [--release vVERSION] [--staged-dir DIR] [--evidence-dir DIR] [--benchmark] [--print-pins] [--inventory-tiers CSV] [--inventory-allow-mutations]\nRuns sequential disposable KVM guests with pinned images, reboot, sudo, package lifecycle and optional warm info timing. Guests run the host architecture by default (--arch overrides, but KVM cannot cross architectures, so a mismatch fails closed instead of silently emulating). With --inventory-tiers, drives tests/cli_behavior_inventory.tsv rows over SSH after a passing lifecycle (see scripts/qemu-inventory.sh). --print-pins lists the pinned guest images without booting anything. Requires Docker, KVM, gh, jq, coreutils. No compilation or host package changes.\n'; exit 0 ;;
    *) printf 'error: unknown argument %s\n' "$1" >&2; exit 2 ;;
  esac
done
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 2
case "$distro" in all|arch|debian|ubuntu|fedora) ;; *) exit 2 ;; esac
if [[ -z "$arch" ]]; then
  case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) printf 'error: host architecture %s is not a QEMU guest architecture\n' "$(uname -m)" >&2; exit 2 ;;
  esac
fi
case "$arch" in x86_64|amd64) arch=x86_64 ;; aarch64|arm64) arch=aarch64 ;; *) exit 2 ;; esac
[[ -z "$staged_dir" || -d "$staged_dir" ]] || exit 2
tsv="$here/../tests/cli_behavior_inventory.tsv"
if [[ -n "$inventory_tiers" && ! -f "$tsv" ]]; then
  printf 'error: --inventory-tiers needs %s\n' "$tsv" >&2
  exit 2
fi
command -v jq >/dev/null || exit 3
source_kind=published
[[ -z "$staged_dir" ]] || source_kind=staged
mkdir -p "$root"
root=$(cd "$root" && pwd)
# Pinned guest images per distro+arch. Hashes are verified against the
# publisher checksum files (Debian SHA512SUMS, Ubuntu SHA256SUMS, Fedora
# CHECKSUM, Arch .SHA256 sidecar). Arch publishes x86_64 cloud images
# only, so arch+aarch64 fails closed in pins_for.
pins_for() {
  local pin_distro=$1 pin_arch=$2
  hash_tool=sha256sum
  firmware=bios
  ssh_service=sshd
  qemu_bin=qemu-system-x86_64
  qemu_machine=q35
  qemu_pkg=qemu-system-x86
  firmware_pkg=ovmf
  firmware_code=/usr/share/OVMF/OVMF_CODE_4M.fd
  firmware_vars_src=/usr/share/OVMF/OVMF_VARS_4M.fd
  guest_uname=x86_64
  controller_image=debian:bookworm@sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931
  case "$pin_distro-$pin_arch" in
    arch-x86_64)
      image_url=https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg-20260901.583572.qcow2
      image_hash=e3e688f97a71b265ce202905a504253f60f3680cf57d011a45411c43bedfa930
      firmware=uefi ;;
    arch-aarch64)
      printf 'error: no aarch64 cloud image pinned for arch (upstream publishes x86_64 only)\n' >&2
      return 1 ;;
    debian-x86_64)
      image_url=https://cloud.debian.org/images/cloud/bookworm/20260903-2590/debian-12-generic-amd64-20260903-2590.qcow2
      image_hash=804377dd07318360c39a75e57b326243442a43bae1e12b33d5f490a64713c15c080a0323cb55d52b381139db9187702d38f36b8b142b8ef36da1031d9de41c2d
      hash_tool=sha512sum
      ssh_service=ssh ;;
    debian-aarch64)
      image_url=https://cloud.debian.org/images/cloud/bookworm/20260903-2590/debian-12-generic-arm64-20260903-2590.qcow2
      image_hash=b0144c1c8e09b187b54af300c8ffc22f17b318d0aa6f5a2caba13f3102441572badbeb098458e599b6897bc80dad50fd0094d6e4b9da9f4a2bd63a8f4c99dea5
      hash_tool=sha512sum
      ssh_service=ssh ;;
    ubuntu-x86_64)
      image_url=https://cloud-images.ubuntu.com/noble/20260826/noble-server-cloudimg-amd64.img
      image_hash=d0fe84bb5f80853425fa6be28e2c106f30104c3cfe8611933f2e65c9b63f0e30
      ssh_service=ssh ;;
    ubuntu-aarch64)
      image_url=https://cloud-images.ubuntu.com/noble/20260826/noble-server-cloudimg-arm64.img
      image_hash=afa139bac6f2629c1e1f2f8f34215f3a9ad9779801bcb945521ba1a45016743f
      ssh_service=ssh ;;
    fedora-x86_64)
      image_url=https://download.fedoraproject.org/pub/fedora/linux/releases/44/Cloud/x86_64/images/Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2
      image_hash=28680fe5b371a5a82ebf43a31926e086a168e59949d03969c5093e7071f90b7f ;;
    fedora-aarch64)
      image_url=https://download.fedoraproject.org/pub/fedora/linux/releases/44/Cloud/aarch64/images/Fedora-Cloud-Base-Generic-44-1.7.aarch64.qcow2
      image_hash=55c60a3b80d3616a08705afd0459e75fe9f03c54aba7a46e4002a41a72fa0d5b ;;
    *) printf 'error: unknown distro/arch %s/%s\n' "$pin_distro" "$pin_arch" >&2; return 1 ;;
  esac
  if [[ "$pin_arch" == aarch64 ]]; then
    # ARM cloud images boot UEFI only; the virt machine needs AAVMF firmware
    # and the ARM system emulator in the controller. The controller image
    # is pinned by arm64 per-arch digest (not the x86_64-era list digest),
    # so the pull is arch-correct by construction on arm64 runners.
    firmware=uefi
    qemu_bin=qemu-system-aarch64
    qemu_machine=virt
    qemu_pkg=qemu-system-arm
    firmware_pkg=qemu-efi-aarch64
    firmware_code=/usr/share/AAVMF/AAVMF_CODE.fd
    firmware_vars_src=/usr/share/AAVMF/AAVMF_VARS.fd
    guest_uname=aarch64
    controller_image=debian:bookworm@sha256:5eac3978974cfa26a880057766c683e55c5763355d30a8beecbd263e0e1621d9
  fi
}

# Lifecycle case ids carry the arch only for aarch64: x86_64 keeps the
# legacy qemu-<distro>-lifecycle id so existing evidence and [qa] issue
# fingerprints stay stable.
case_suffix=""
[[ "$arch" == aarch64 ]] && case_suffix="-aarch64"

if [[ "$print_pins" == true ]]; then
  # Audit mode: list every pinned image without booting anything.
  # Honors --distro; always covers both arches so pin reviews see the
  # whole table (unsupported combos like arch/aarch64 print nothing).
  for pin_distro in arch debian ubuntu fedora; do
    [[ "$distro" == all || "$distro" == "$pin_distro" ]] || continue
    for pin_arch in x86_64 aarch64; do
      if pins_for "$pin_distro" "$pin_arch" 2>/dev/null; then
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$pin_distro" "$pin_arch" "$image_url" "$image_hash" "$hash_tool" "$firmware"
      fi
    done
  done
  exit 0
fi
if [[ "$distro" == all ]]; then
  suite=$(mktemp -d "$root/suite-XXXXXX")
  rc=0
  args=(--release "$tag" --arch "$arch")
  [[ -z "$staged_dir" ]] || args+=(--staged-dir "$staged_dir")
  [[ "$benchmark" == false ]] || args+=(--benchmark)
  [[ -z "$inventory_tiers" ]] || args+=(--inventory-tiers "$inventory_tiers")
  [[ "$inventory_mutations" == false ]] || args+=(--inventory-allow-mutations)
  jq -n --arg source "$source_kind" --arg suffix "$case_suffix" '["arch", "debian", "ubuntu", "fedora"] | map({case_id:("qemu-"+.+$suffix+"-lifecycle"), distro:., result:"NOT_RUN", artifact_source:$source, exit_code:null, elapsed_seconds:0})' > "$suite/results.json"
  for target in arch debian ubuntu fedora; do
    jq --arg target "$target" 'map(if .distro == $target then .result = "INCOMPLETE" else . end)' "$suite/results.json" > "$suite/results.next.json"
    mv "$suite/results.next.json" "$suite/results.json"
    "$0" --distro "$target" --evidence-dir "$suite/$target" "${args[@]}" || rc=1
    reports=("$suite/$target"/run-*/results.json)
    if [[ ${#reports[@]} -eq 1 && -f "${reports[0]}" ]] && jq -e --arg target "$target" --arg suffix "$case_suffix" 'length == 1 and .[0].distro == $target and .[0].case_id == ("qemu-"+$target+$suffix+"-lifecycle")' "${reports[0]}" >/dev/null; then
      jq --arg target "$target" --slurpfile report "${reports[0]}" 'map(if .distro == $target then $report[0][0] else . end)' "$suite/results.json" > "$suite/results.next.json"
      mv "$suite/results.next.json" "$suite/results.json"
    else rc=1; fi
  done
  printf 'Suite evidence: %s\n' "$suite"
  exit "$rc"
fi
case_id="qemu-${distro}${case_suffix}-lifecycle"
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d "$root/run-XXXXXX")
controller="omg-qemu-${work##*/}"
printf 'Starting %s (%s). Evidence: %s\n' "$distro" "$arch" "$work"
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
  jq -n --arg distro "$distro" --arg case_id "$case_id" --arg result "$result" --arg source "$source_kind" --argjson rc "$rc" --argjson elapsed "$SECONDS" \
    '[{case_id:$case_id,distro:$distro,result:$result,artifact_source:$source,exit_code:$rc,elapsed_seconds:$elapsed}]' > "$work/results.json"
  timeout --kill-after=2s 12s env OMG_SMOKE_RELEASE="$tag" "$repo_root/scripts/report-smoke-sentry.sh" "$work/results.json" > "$work/reporting.log" 2>&1 || true
  printf '%s %s. Evidence: %s\n' "$distro" "$result" "$work"
  exit "$rc"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$work/release" "$work/guest"
: > "$work/guest/serial.log"
# Preflight probes. Each fails closed to HARNESS_ERROR (exit 3 lands in
# the EXIT trap, which records the result): a missing pin, missing KVM,
# or a host/guest arch mismatch must never silently fall back to TCG.
pins_for "$distro" "$arch" || exit 3
kvm_device="${OMG_QEMU_KVM_DEVICE:-/dev/kvm}"
if [[ "${OMG_QEMU_ALLOW_NO_KVM:-0}" != 1 ]]; then
  if [[ ! -c "$kvm_device" ]]; then
    printf 'error: KVM device %s is missing; refusing TCG fallback\n' "$kvm_device" >&2
    printf 'kvm=missing device=%s\n' "$kvm_device" > "$work/kvm-probe.log"
    exit 3
  fi
  if [[ ! -r "$kvm_device" || ! -w "$kvm_device" ]]; then
    printf 'error: KVM device %s is not accessible\n' "$kvm_device" >&2
    printf 'kvm=inaccessible device=%s\n' "$kvm_device" > "$work/kvm-probe.log"
    exit 3
  fi
  printf 'kvm=ok device=%s\n' "$kvm_device" > "$work/kvm-probe.log"
else
  printf 'kvm=skipped device=%s (OMG_QEMU_ALLOW_NO_KVM=1; tests only)\n' "$kvm_device" > "$work/kvm-probe.log"
fi
host_arch=x86_64
case "$(uname -m)" in aarch64|arm64) host_arch=aarch64 ;; esac
if [[ "$host_arch" != "$arch" ]]; then
  printf 'error: guest arch %s needs a %s host with KVM; refusing cross-architecture emulation\n' "$arch" "$arch" >&2
  exit 3
fi
if [[ "$arch" == aarch64 && -z "$staged_dir" ]]; then
  # The release workflow publishes x86_64-linux archives (plus
  # aarch64-darwin for macOS) but no aarch64-linux archives, so a
  # published ARM leg has nothing to download. Staged ARM builds on
  # arm64 runners are the supported path.
  printf 'error: no published aarch64-linux archives; use --staged-dir with arm64-built binaries\n' >&2
  exit 3
fi
for tool in docker timeout sha256sum; do command -v "$tool" >/dev/null || exit 3; done
[[ -n "$staged_dir" ]] || { command -v gh >/dev/null || exit 3; }
timeout --kill-after=2s 15s docker version --format '{{.Server.Version}}' > "$work/engine-preflight.log" 2>&1 || exit 3
{ date -u; uname -a; cat /proc/loadavg; grep -E 'MemTotal|MemAvailable|SwapFree' /proc/meminfo; } > "$work/host-metadata.txt"
archive="omg-${tag}-${arch}-linux-${distro}.tar.gz"
if [[ -n "$staged_dir" ]]; then
  cp "$staged_dir/$archive" "$staged_dir/$archive.sha256" "$work/release/"
else
  timeout 120 gh release download "$tag" --repo PyRo1121/omg --pattern "$archive" --pattern "$archive.sha256" --dir "$work/release"
fi
read -r digest filename extra < "$work/release/$archive.sha256"
[[ "$digest" =~ ^[0-9a-f]{64}$ && "$filename" == "$archive" && -z "${extra:-}" ]]
[[ $(wc -l < "$work/release/$archive.sha256") -eq 1 ]]
(cd "$work/release" && sha256sum -c "$archive.sha256") > "$work/release-checksum.txt"
printf 'distro=%s\narch=%s\nrelease=%s\nartifact_source=%s\nimage_url=%s\nimage_digest=%s\nfirmware=%s\nqemu=%s -machine %s\ncontroller=%s\ncase_id=%s\n' "$distro" "$arch" "$tag" "$source_kind" "$image_url" "$image_hash" "$firmware" "$qemu_bin" "$qemu_machine" "$controller_image" "$case_id" > "$work/metadata.txt"
started=true
timeout 120 docker run -d --name "$controller" --cpus 2 --memory 3g --memory-swap 3g --device /dev/kvm \
  --mount "type=bind,src=$work,dst=/work" --workdir /work \
  "$controller_image" sleep infinity > "$work/controller-id.txt"
timeout 300 docker exec "$controller" sh -c "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends $qemu_pkg qemu-utils cloud-image-utils openssh-client curl ca-certificates $firmware_pkg jq" > "$work/controller-setup.log" 2>&1
timeout 360 docker exec "$controller" bash -c 'set -e; cd /work/guest; curl --fail --location --max-time 300 -o base.qcow2 "$1"; printf "%s  base.qcow2\n" "$2" | "$3" -c -; "$4" --version; qemu-img info base.qcow2' _ "$image_url" "$image_hash" "$hash_tool" "$qemu_bin" > "$work/image-setup.log" 2>&1
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
  cp "$4" vars.fd
  firmware=(-drive if=pflash,format=raw,readonly=on,file="$3" -drive if=pflash,format=raw,file=vars.fd)
fi
"$5" -machine "$6,accel=kvm" -cpu host -smp 2 -m 1536 \
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
timeout 700 docker exec "$controller" bash /work/boot.sh "$firmware" "$ssh_service" "$firmware_code" "$firmware_vars_src" "$qemu_bin" "$qemu_machine" > "$work/boot.log" 2>&1
if [[ -n "$inventory_tiers" ]]; then
  # The inventory executor runs inside the controller (same netns as the
  # guest); /work is bind-mounted there.
  cp "$here/qemu-inventory.sh" "$work/qemu-inventory.sh"
  cp "$tsv" "$work/cases.tsv"
fi
cat > "$work/guest-check.sh" <<'GUEST'
#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C NO_COLOR=1
cd "$HOME"
distro=$1; tag=$2; digest=$3; benchmark=$4; guest_arch=$5; expected_uname=$6
actual_id=$(awk -F= '$1 == "ID" {gsub(/"/, "", $2); print $2}' /etc/os-release)
[[ "$actual_id" == "$distro" && $(uname -m) == "$expected_uname" ]] || exit 120
mkdir -p evidence
trap 'status=$?; printf "%s\n" "$status" > evidence/exit-code' EXIT
printf '%s  release.tar.gz\n' "$digest" | sha256sum -c -
tar -xzf release.tar.gz
bin="$HOME/omg-${tag}-${guest_arch}-linux-${distro}/omg"
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
timeout 600 docker exec -w /work/guest "$controller" ssh "${opts[@]}" -p 2222 bench@127.0.0.1 "bash guest-check.sh '$distro' '$tag' '$digest' '$benchmark' '$arch' '$guest_uname'" > "$work/guest-check.log" 2>&1 || rc=$?
timeout 60 docker exec -w /work/guest "$controller" scp -r "${opts[@]}" -P 2222 bench@127.0.0.1:evidence /work/guest/ > "$work/evidence-copy.log" 2>&1
guest_rc=$(<"$work/guest/evidence/exit-code")
if [[ ! "$guest_rc" =~ ^[0-9]+$ || "$guest_rc" != "$rc" ]]; then
  printf 'Guest exit %s differs from transport exit %s\n' "$guest_rc" "$rc" >&2
  exit 3
fi
if [[ -n "$inventory_tiers" && "$rc" == 0 ]]; then
  # TSV-driven rows run only on a healthy guest, inside the controller
  # (same netns; /work is bind-mounted). Evidence lands in $work/inventory.
  inv_args=(--work /work --distro "$distro" --tiers "$inventory_tiers" --tag "$tag"
    --binary "/home/bench/omg-${tag}-${arch}-linux-${distro}/omg" --tsv /work/cases.tsv)
  [[ "$inventory_mutations" == false ]] || inv_args+=(--allow-mutations)
  if ! timeout 1800 docker exec -w /work "$controller" bash /work/qemu-inventory.sh "${inv_args[@]}" > "$work/inventory.log" 2>&1; then
    printf 'Inventory rows failed; see %s/inventory\n' "$work" >&2
    rc=1
  fi
fi
case "$rc" in 0) result=PASS ;; 120|124|125|126|127|137|255) result=HARNESS_ERROR ;; *) result=PRODUCT_FAIL ;; esac
exit "$rc"
