---
title: Fast Status Binary Protocol
sidebar_position: 34
description: Deep dive into the zero-IPC status file
---

# Fast Status Binary Protocol

**How OMG Achieves Sub-Millisecond Status Reads**

This guide explains the technical details behind OMG's lightning-fast status queries, which power shell prompts and CLI commands.

---

## 🎯 The Problem

Traditional package managers like `pacman` take 10-50ms to count installed packages because they:
1. Parse text-based database files
2. Walk directory structures
3. Execute system calls for each query

For shell prompts that run on every command, this adds noticeable lag.

---

## ⚡ OMG's Solution: Binary Status File

OMG uses a **fixed-size binary snapshot** that can be read in \u003c1ms without:
- IPC overhead (no socket connection)
- Parsing (raw memory-mapped bytes)
- System calls (single file read)

### The Format

The status file (`$XDG_RUNTIME_DIR/omg.status`) is exactly **32 bytes**:

```
Offset | Size | Field                | Type | Description
-------|------|----------------------|------|----------------------------------
0      | 4    | magic                | u32  | 0x4F4D4753 ("OMGS" in ASCII)
4      | 1    | version              | u8   | Format version (currently 1)
5      | 3    | pad                  | u8[] | Alignment padding (zeros)
8      | 4    | total_packages       | u32  | Total installed packages
12     | 4    | explicit_packages    | u32  | User-installed packages
16     | 4    | orphan_packages      | u32  | Unused dependencies
20     | 4    | updates_available    | u32  | Pending updates
24     | 8    | timestamp            | u64  | Unix timestamp (seconds)
```

### Why This Format?

1. **Fixed Size**: 32 bytes fits in a single CPU cache line
2. **Aligned**: All fields are naturally aligned for zero-copy reads
3. **Validated**: Magic number prevents reading corrupt data
4. **Versioned**: Future-proof for format changes
5. **Atomic**: Written via temp file + rename (POSIX atomic operation)

---

## 🔄 Update Lifecycle

### 1. Daemon Writes (Every 5 Minutes)

```rust
// In daemon background worker
let status = FastStatus::new(total, explicit, orphans, updates);
status.write_to_file(&status_path)?;
```

The daemon:
1. Queries package manager for current counts
2. Creates a `FastStatus` struct
3. Writes to temporary file (`omg.status.tmp`)
4. Atomically renames to `omg.status`

**Why atomic rename?** Ensures readers never see partial/corrupt data.

### 2. Shell Hook Reads (Every Prompt)

The Zsh hook includes this function:

```bash
_omg_refresh_cache() {
  local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"
  [[ -f "$f" ]] || return
  local now=$EPOCHSECONDS
  # Only refresh every 60 seconds
  (( now - _OMG_CACHE_TIME < 60 )) && return
  _OMG_CACHE_TIME=$now
  # Read all values at once using od (octal dump)
  local data=$(od -An -j8 -N16 -tu4 "$f" 2>/dev/null)
  read _OMG_TOTAL _OMG_EXPLICIT _OMG_ORPHANS _OMG_UPDATES <<< "$data"
}
```

This uses the `od` command to:
- Skip magic/version/padding (`-j8` = jump 8 bytes)
- Read 16 bytes (`-N16`)
- Interpret as unsigned 32-bit integers (`-tu4`)

Then `omg-ec`, `omg-tc`, etc. just echo the cached shell variables (sub-microsecond).

### 3. Direct File Read (omg-fast)

For non-shell contexts, the `omg-fast` binary reads directly:

```rust
pub fn read_explicit_count() -> Option<usize> {
    let path = paths::runtime_dir()?.join("omg.status");
    let mut file = File::open(path).ok()?;

    // Seek to offset 12 (skip magic, version, pad, total)
    file.seek(SeekFrom::Start(12)).ok()?;

    // Read single u32
    let mut bytes = [0u8; 4];
    file.read_exact(&mut bytes).ok()?;

    Some(u32::from_ne_bytes(bytes) as usize)
}
```

This achieves ~1ms latency by:
- Single `open()` system call
- Single `read()` at known offset
- Zero parsing/allocation

---

## 📊 Performance Comparison

| Method | Latency | Use Case |
|--------|---------|----------|
| **Shell variable** (`omg-ec`) | \u003c1μs | Prompts (cached) |
| **Direct file read** (`omg-fast ec`) | ~1ms | Scripts |
| **IPC query** (`omg explicit --count`) | ~1.2ms | Commands |
| **System query** (`pacman -Qq \| wc -l`) | ~14ms | Fallback |

### Why So Fast?

1. **No Process Spawning**: Shell variables are in-process
2. **No Parsing**: Binary format is direct memory interpretation
3. **Single Syscall**: One `open()` + one `read()`
4. **Cache-Friendly**: 32 bytes fits in L1 cache

---

## 🛡️ Reliability Features

### Staleness Detection

The file includes a timestamp. Readers reject data older than 60 seconds:

```rust
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)?
    .as_secs();

if now.saturating_sub(status.timestamp) > 60 {
    return None; // Too old, fall back to IPC
}
```

### Corruption Detection

The magic number validates file integrity:

```rust
if status.magic != 0x4F4D4753 {
    return None; // Corrupted or wrong file
}
```

### Version Compatibility

The version field allows graceful handling of format changes:

```rust
if status.version != 1 {
    return None; // Unknown format version
}
```

---

## 🔬 Advanced: Memory-Mapped Alternative

For even faster reads, you can memory-map the file safely using `zerocopy`:

```rust
use memmap2::Mmap;
use zerocopy::FromBytes;

pub fn read_mmap() -> Option<FastStatus> {
    let file = File::open(status_path()).ok()?;
    let mmap = unsafe { Mmap::map(&file).ok()? }; // Mmap itself is safe if file is not modified

    // SAFE zero-copy: use zerocopy to parse from mmap buffer
    let status = FastStatus::read_from_bytes(&mmap).ok()?;

    // Validate
    if status.magic == MAGIC && status.version == VERSION {
        Some(*status)
    } else {
        None
    }
}
```

This eliminates the `read()` syscall entirely (after initial `mmap()`), achieving sub-microsecond latency without manual `unsafe` pointer casting.

---

## 📁 File Location

The status file is stored in:

```
$XDG_RUNTIME_DIR/omg.status  (preferred, tmpfs)
/tmp/omg.status              (fallback)
```

**Why `/run/user/$UID`?**
- **tmpfs**: RAM-backed filesystem (no disk I/O)
- **User-isolated**: Automatic cleanup on logout
- **Fast**: No disk latency

---

## 🔧 Troubleshooting

### Status File Missing

```bash
# Check if daemon is running
pgrep omgd

# If not, start it
omg daemon

# Verify file exists
ls -la $XDG_RUNTIME_DIR/omg.status
```

### Stale Data

```bash
# Check timestamp (offset 24, 8 bytes)
od -An -j24 -N8 -tu8 $XDG_RUNTIME_DIR/omg.status

# Compare to current time
date +%s
```

If timestamp is \u003e60 seconds old, the daemon may be frozen.

### Corruption

```bash
# Check magic number (should be 1397048659 = 0x4F4D4753)
od -An -j0 -N4 -tu4 $XDG_RUNTIME_DIR/omg.status

# Should output: 1397048659
```

If not, delete the file and restart daemon:

```bash
rm $XDG_RUNTIME_DIR/omg.status
pkill omgd && omg daemon
```

---

## 📚 Implementation References

### Source Files

- **Format Definition**: `src/core/fast_status.rs`
- **Writer (Daemon)**: `src/daemon/server.rs` (background worker)
- **Reader (CLI)**: `src/bin/omg-fast.rs`
- **Shell Functions**: `src/hooks/mod.rs` (ZSH_HOOK, BASH_HOOK)

### Related Docs

- [Daemon Internals](./daemon.md) — Background worker lifecycle
- [Shell Integration](./shell-integration.md) — Hook system details
- [Architecture](./architecture.md) — Overall system design

---

## 💡 Key Takeaways

1. **Binary over Text**: 100x faster than parsing text files
2. **Fixed Size**: Enables zero-allocation reads
3. **Atomic Writes**: Ensures data integrity
4. **Validation**: Magic + version + timestamp prevent errors
5. **tmpfs Location**: Eliminates disk I/O entirely

This design is why `omg-ec` can update your shell prompt with zero perceptible lag, even on every keystroke.
