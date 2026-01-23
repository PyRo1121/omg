---
title: API Reference
sidebar_position: 100
description: Daemon IPC Protocol and API documentation
---

# API Reference

**Internal Daemon IPC Protocol and Data Structures**

This document details the binary IPC protocol used by the `omg` client to communicate with the `omgd` daemon.

---

## 🔌 Connection Details

- **Transport**: Unix Domain Socket
- **Default Path**: `/run/user/1000/omg.sock` (or `$XDG_RUNTIME_DIR/omg.sock`)
- **Framing**: Length-delimited (4-byte big-endian length prefix)
- **Serialization**: `bitcode` (Binary)

---

## 📥 Request Types

Requests are sent as a `Request` enum serialized with `bitcode`.

### `Search`
Performs a fast package search across system and runtime backends.
- **Fields**:
  - `id`: `RequestId` (u64)
  - `query`: `String`
  - `limit`: `Option<usize>`

### `Info`
Retrieves detailed metadata for a specific package.
- **Fields**:
  - `id`: `RequestId`
  - `package`: `String`

### `Status`
Returns the current system "vital signs" (package counts, updates, vulnerabilities).
- **Fields**:
  - `id`: `RequestId`

### `SecurityAudit`
Triggers a comprehensive vulnerability scan across all installed packages.
- **Fields**:
  - `id`: `RequestId`

### `Batch`
Combines multiple requests into a single IPC round-trip for maximum efficiency.
- **Fields**:
  - `id`: `RequestId`
  - `requests`: `Vec<Request>`

### `Suggest`
Retrieves fuzzy-matched suggestions for a package name.
- **Fields**:
  - `id`: `RequestId`
  - `query`: `String`
  - `limit`: `Option<usize>`

---

## 📤 Response Types

Responses are returned as a `Response` enum.

### `Success`
Indicates the request was processed successfully.
- **Fields**:
  - `id`: `RequestId`
  - `result`: `ResponseResult`

### `Error`
Indicates a failure occurred during processing.
- **Fields**:
  - `id`: `RequestId`
  - `code`: `i32`
  - `message`: `String`

---

## 📊 Data Structures

### `PackageInfo` (Search Result)
```rust
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
}
```

### `DetailedPackageInfo`
```rust
pub struct DetailedPackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: String,
    pub size: u64,
    pub download_size: u64,
    pub repo: String,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
    pub source: String,
}
```

### `StatusResult`
```rust
pub struct StatusResult {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
    pub security_vulnerabilities: usize,
    pub runtime_versions: Vec<(String, String)>,
}
```

---

## ⚠️ Error Codes

| Code | Constant | Description |
|------|----------|-------------|
| `-32700` | `PARSE_ERROR` | Failed to deserialize request |
| `-32601` | `METHOD_NOT_FOUND` | Unknown request type |
| `-32602` | `INVALID_PARAMS` | Invalid arguments (e.g., query too long) |
| `-32603` | `INTERNAL_ERROR` | Unexpected daemon failure |
| `-1001` | `PACKAGE_NOT_FOUND` | The requested package does not exist |
| `-1002` | `RATE_LIMITED` | Too many requests from this client |

---

## 🚀 Performance Tips

1. **Use `Batch`**: If you need multiple pieces of data (e.g., status and explicit counts), wrap them in a `Batch` request to avoid multiple kernel context switches.
2. **Reuse IDs**: Track `RequestId` to match responses to requests in asynchronous client implementations.
3. **Limit Queries**: Use the `limit` field in `Search` and `Suggest` to reduce payload size.
