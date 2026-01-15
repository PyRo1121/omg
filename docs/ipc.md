# IPC Protocol

OMG uses a custom binary IPC protocol over Unix domain sockets for maximum performance. The protocol uses length-delimited framing with bincode serialization for sub-millisecond latency.

## Transport Layer

### Unix Domain Sockets

Unix domain sockets provide the foundation for OMG's IPC:

- **Path**: `$XDG_RUNTIME_DIR/omg.sock` or `/tmp/omg.sock`
- **Type**: `SOCK_STREAM` (TCP-like, reliable, ordered)
- **Permissions**: `0o600` (user read/write only)
- **Advantages**:
  - No network stack overhead (kernel-level bypass)
  - File system permissions for security
  - Automatic connection tracking
  - Zero-copy data transfer in some cases

### Length-Delimited Framing

The protocol uses `tokio_util::codec::LengthDelimitedCodec`:

```rust
let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
```

Framing format:
- **4-byte header**: Big-endian `u32` indicating message length
- **Variable body**: Bincode-serialized request/response
- **Maximum frame**: 8MB (configurable in codec)
- **Advantages**:
  - Clear message boundaries
  - Stream multiplexing support
  - Memory-efficient parsing
  - Backpressure handling

### Synchronous Optimization

For synchronous operations, OMG uses a custom optimization:

```rust
// Combined length + payload to save syscalls
let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
send_buf.extend_from_slice(&len.to_be_bytes());
send_buf.extend_from_slice(&request_bytes);
stream.write_all(&send_buf)?;
```

This reduces system calls from 2 to 1 per message, critical for sub-millisecond operations.

## Serialization Format

### Bincode Choice

Bincode is selected for serialization due to:

- **Performance**: No schema overhead, direct binary mapping
- **Size**: Compact representation (no field names)
- **Speed**: Zero-copy deserialization where possible
- **Safety**: Compile-time type checking

### Serialization Performance

Typical serialization metrics:
- **Search Request**: ~50 bytes serialized in ~5μs
- **Search Response**: ~2KB serialized in ~50μs
- **Info Response**: ~500 bytes serialized in ~10μs
- **Status Response**: ~1KB serialized in ~20μs

## Protocol Types

### Request Structure

All requests share a common pattern:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Search {
        id: RequestId,
        query: String,
        limit: Option<usize>,
    },
    Info {
        id: RequestId,
        package: String,
    },
    Status {
        id: RequestId,
    },
    Explicit {
        id: RequestId,
    },
    SecurityAudit {
        id: RequestId,
    },
    Ping {
        id: RequestId,
    },
    CacheStats {
        id: RequestId,
    },
    CacheClear {
        id: RequestId,
    },
}
```

#### Request ID Management

```rust
pub struct DaemonClient {
    request_id: AtomicU64,
}

// ID generation
let id = self.request_id.fetch_add(1, Ordering::SeqCst);
```

- **Type**: `u64` (atomic increment)
- **Initial value**: 1
- **Overflow**: Wraps around (practically impossible)
- **Ordering**: Sequential consistency guarantees

### Response Structure

Responses follow a unified success/error pattern:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Success {
        id: RequestId,
        result: ResponseResult,
    },
    Error {
        id: RequestId,
        code: i32,
        message: String,
    },
}
```

#### Response Result Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseResult {
    Search(SearchResult),
    Info(DetailedPackageInfo),
    Status(StatusResult),
    Explicit(ExplicitResult),
    SecurityAudit(SecurityAuditResult),
    Ping(String),
    CacheStats { size: usize, max_size: usize },
    Message(String),
}
```

### Error Codes

Standard error codes follow JSON-RPC conventions:

```rust
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;      // Invalid JSON
    pub const METHOD_NOT_FOUND: i32 = -32601; // Method doesn't exist
    pub const INVALID_PARAMS: i32 = -32602;   // Invalid method parameters
    pub const INTERNAL_ERROR: i32 = -32603;   // Internal JSON-RPC error
    pub const PACKAGE_NOT_FOUND: i32 = -1001; // Custom: Package not found
}
```

## Client Implementation

### Async Client

The async client uses Tokio's framed streams:

```rust
pub async fn call(&mut self, request: Request) -> Result<ResponseResult> {
    let id = request.id();
    let framed = self.framed.as_mut()?;
    
    // Send request
    let request_bytes = bincode::serialize(&request)?;
    framed.send(request_bytes.into()).await?;
    
    // Receive response
    let response_bytes = framed.next().await.ok_or("Disconnected")??;
    let response: Response = bincode::deserialize(&response_bytes)?;
    
    // Validate ID
    match response {
        Response::Success { id: resp_id, result } => {
            if resp_id != id {
                bail!("Request ID mismatch: sent {id}, got {resp_id}");
            }
            Ok(result)
        }
        Response::Error { id: resp_id, code, message } => {
            if resp_id != id {
                bail!("Response ID mismatch");
            }
            bail!("RPC error {}: {}", code, message);
        }
    }
}
```

### Sync Client

The sync client optimizes for latency:

```rust
pub fn call_sync(&mut self, request: Request) -> Result<ResponseResult> {
    let stream = self.sync_stream.as_mut()?;
    
    // Combined write for single syscall
    let request_bytes = bincode::serialize(&request)?;
    let len = request_bytes.len() as u32;
    let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
    send_buf.extend_from_slice(&len.to_be_bytes());
    send_buf.extend_from_slice(&request_bytes);
    stream.write_all(&send_buf)?;
    
    // Read length then payload
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes)?;
    
    // Deserialize and validate
    let response: Response = bincode::deserialize(&resp_bytes)?;
    // ... ID validation as above
}
```

### Connection Management

#### Connection Pooling

The client doesn't currently pool connections but could be extended:
- **Single Connection**: Per-process client instance
- **Reconnection**: Automatic on socket errors
- **Timeout**: Configurable connection timeout
- **Backoff**: Exponential backoff for failed connections

#### Connection Lifecycle

1. **Connect**: Resolve socket path, attempt connection
2. **Handshake**: No explicit handshake (first message validates)
3. **Request/Response**: Arbitrary number of exchanges
4. **Disconnect**: On drop or explicit close

## Performance Characteristics

### Latency Breakdown

Typical operation latencies (cached results):

| Operation | Serialize | Send | Process | Receive | Deserialize | Total |
|-----------|-----------|------|---------|---------|-------------|-------|
| Search    | 5μs       | 10μs | 100μs   | 10μs    | 50μs        | 175μs |
| Info      | 2μs       | 5μs  | 50μs    | 5μs     | 10μs        | 72μs  |
| Status    | 3μs       | 5μs  | 75μs    | 5μs     | 20μs        | 108μs |
| Ping      | 1μs       | 5μs  | 10μs    | 5μs     | 2μs         | 23μs  |

### Throughput

- **Max Requests/Second**: ~10,000 (limited by daemon processing)
- **Bandwidth Usage**: ~1MB/s at full load
- **Memory Overhead**: ~1KB per in-flight request
- **CPU Usage**: <1% for serialization/deserialization

### Optimization Techniques

1. **Zero-Copy**: Where possible, avoid allocations
2. **Batching**: Could batch multiple requests (not implemented)
3. **Compression**: Not worth it for small messages
4. **Caching**: Client-side caching reduces round trips

## Protocol Extensions

### Versioning

The protocol doesn't currently version but could use:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    V1(RequestV1),
    V2(RequestV2),
}
```

### Backward Compatibility

Future considerations:
- **Field Addition**: Bincode handles optional fields
- **Enum Variants**: Adding variants is safe
- **Struct Changes**: Require version bump
- **Error Codes**: New codes won't break old clients

### Authentication

Could be extended with:
```rust
pub enum Request {
    Auth {
        token: String,
        request: Box<Request>,
    },
    // ... other variants
}
```

## Security Considerations

### Socket Security

- **Permissions**: File system restricts access
- **Location**: User-specific runtime directory
- **Symlink Protection**: Kernel prevents symlink attacks
- **No Network**: Cannot be accessed remotely

### Data Protection

- **No Secrets**: Protocol doesn't transmit sensitive data
- **Local Only**: Unix sockets are machine-local
- **No Encryption**: Unnecessary for local communication
- **No Authentication**: Relies on OS permissions

### DoS Protection

Current limitations:
- **No Rate Limiting**: Could be overwhelmed
- **No Size Limits**: Except codec's 8MB default
- **No Timeouts**: Connections can be held open
- **No Authentication**: Any user can connect

Future enhancements:
- **Rate Limiting**: Per-client request limits
- **Connection Limits**: Maximum concurrent clients
- **Timeouts**: Idle connection termination
- **Authentication**: Token-based or Unix credentials

## Debugging and Monitoring

### Message Inspection

For debugging, the protocol can be inspected:

```rust
// Enable debug logging
tracing::debug!("Sending request: {:?}", request);
tracing::debug!("Received response: {:?}", response);

// Raw bytes (hex dump)
tracing::trace!("Raw request: {:02X?}", request_bytes);
```

### Performance Monitoring

Key metrics to track:
- **Round-trip latency**: Per-operation timing
- **Message size**: Payload distribution
- **Error rates**: Per-error-type frequency
- **Connection churn**: Connect/disconnect frequency

### Common Issues

1. **Socket Not Found**: Daemon not running
2. **Permission Denied**: Wrong user or permissions
3. **Connection Reset**: Daemon restarted
4. **ID Mismatch**: Client bug or daemon issue
5. **Parse Error**: Version mismatch or corruption

## Testing

### Unit Tests

Protocol components are tested:
```rust
#[test]
fn test_request_serialization() {
    let request = Request::Search {
        id: 1,
        query: "test".to_string(),
        limit: Some(10),
    };
    let bytes = bincode::serialize(&request).unwrap();
    let decoded: Request = bincode::deserialize(&bytes).unwrap();
    assert_eq!(request.id(), decoded.id());
}
```

### Integration Tests

End-to-end IPC testing:
- Start test daemon
- Connect client
- Send all request types
- Verify responses
- Test error conditions

### Performance Tests

Benchmark critical paths:
- Serialization/deserialization speed
- Round-trip latency
- Concurrent client handling
- Memory usage under load
- `METHOD_NOT_FOUND = -32601`
- `INVALID_PARAMS = -32602`
- `INTERNAL_ERROR = -32603`
- `PACKAGE_NOT_FOUND = -1001`

## Sync vs Async Client
The client exposes two IPC paths:
- **Async**: Uses Tokio `UnixStream` + `Framed` for multiplexed requests.
- **Sync**: Uses `std::os::unix::net::UnixStream` for sub‑millisecond sync calls.

The sync path sends a big-endian length prefix followed by the serialized payload and expects the same format in return.

Source: `src/core/client.rs`.
