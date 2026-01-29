---
title: "Fast Status Binary Protocol"
description: "Deep dive into the zero-IPC status file"
---
# Fast Status Binary Protocol

**How OMG Achieves Sub-Millisecond Status Reads**

This guide explains the technical details behind OMG's lightning-fast status queries, which power shell prompts and CLI commands.


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


## 💡 Key Takeaways

1. **Binary over Text**: 100x faster than parsing text files
2. **Fixed Size**: Enables zero-allocation reads
3. **Atomic Writes**: Ensures data integrity
4. **Validation**: Magic + version + timestamp prevent errors
5. **tmpfs Location**: Eliminates disk I/O entirely

This design is why `omg-ec` can update your shell prompt with zero perceptible lag, even on every keystroke.
