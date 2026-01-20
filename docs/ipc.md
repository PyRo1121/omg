---
title: IPC Protocol
sidebar_position: 33
description: Binary protocol for CLI-daemon communication
---

# IPC Protocol

OMG uses a custom binary IPC protocol over Unix domain sockets for maximum performance. The protocol uses length-delimited framing with bitcode serialization for sub-millisecond latency.

## ⚡ IPC Architecture

OMG uses a high-performance binary Inter-Process Communication (IPC) protocol designed specifically for sub-millisecond responsiveness between the user interface and the background engine.

### Transport Layer: Unix Domain Sockets
Communication happens exclusively over **Unix Domain Sockets**, which offer several key advantages over network-based protocols:
- **Kernel-Level Speed**: Data is transferred directly within the kernel, bypassing the entire TCP/IP stack.
- **Security by Design**: Access is controlled by standard file system permissions, ensuring that only you (the user) can communicate with your daemon.
- **Reliability**: Ordered and reliable message delivery is guaranteed by the operating system.

---

## 📨 Protocol Design

The protocol is optimized for low latency and high throughput, using a structured binary format.

### Framing Strategy
OMG uses **Length-Delimited Framing**. Every message is prefixed by a 4-byte header that tells the receiver exactly how many bytes to expect. This allows for:
- **Zero-Ambiguity boundaries**: No confusion between multiple messages on the same stream.
- **Memory Efficiency**: The system allocates exactly the right amount of memory for each incoming request.
- **Backpressure Support**: The daemon can signal the CLI to slow down if it becomes overloaded.

### Binary Serialization
We use a high-efficiency binary serializer (**Bitcode**) instead of text-based formats like JSON.
- **Density**: Serialized messages are up to 10x smaller than equivalent JSON.
- **Speed**: Serialization and deserialization take only a few microseconds.
- **Resilience**: The strictly typed nature of the binary format prevents many common communication errors.

---

## 🔄 Interaction Patterns

### Request-Response Lifecycle
Every client interaction follows a predictable path:
1. **CLI Request**: The CLI serializes your command and sends it through the socket.
2. **Daemon Processing**: The daemon parses the request and routes it to the correct handler.
3. **Serialized Response**: The result is sent back, including a unique **Request ID** to ensure responses always match their original requests.
4. **Error Handling**: Every response includes an error code, ranging from standard success (0) to specific network or package-not-found issues.

### Sync vs. Async Paths
OMG provides two optimized paths for different use cases:
- **Fast Synchronous**: Used for simple status checks where speed is everything. We combine the message length and payload into a single operation to minimize system calls.
- **Concurrent Asynchronous**: Used for long-running tasks (like full system updates), allowing the CLI to handle multiple streams or display real-time progress bars.

---

## 📊 Performance Benchmarks

In a typical production environment, the IPC layer introduces virtually zero overhead:

| Operation | Total Round-Trip Time |
|-----------|-----------------------|
| **Ping**  | ~25μs                 |
| **Status**| ~110μs                |
| **Search**| ~180μs (cached)       |
| **Info**  | ~75μs (cached)        |

---

## 🛡️ Security Model

- **Local-Only**: IPC is restricted to the local machine and cannot be accessed over the network.
- **Permission Boundary**: Strict `0600` permissions on the socket file.
- **Tamper Protection**: The binary framing ensures that malformed or malicious messages are rejected immediately before they reach the core engine.
- **Resource Limits**: The daemon enforces maximum frame sizes to protect against denial-of-service through large payloads.
