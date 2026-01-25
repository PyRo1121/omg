---
title: IPC Protocol
sidebar_position: 33
description: Binary protocol for CLI-daemon communication
---

# Private Communication Interface

OMG uses a high-performance private interface designed specifically for sub-millisecond responsiveness between the user interface and the background engine.

## ⚡ Architecture

Communication happens exclusively over a local secure channel, which offer several key advantages over standard network protocols:
- **Direct Speed**: Data is transferred directly within the system kernel, bypassing the entire network stack.
- **Security by Design**: Access is controlled by standard system permissions, ensuring that only you (the user) can communicate with your background engine.
- **Reliability**: Ordered and reliable message delivery is guaranteed by the operating system.

---

## 📨 Protocol Design

The protocol is optimized for low latency and high throughput, using a structured binary format.

### Framing Strategy
OMG uses **Length-Delimited Framing**. Every message is prefixed by a **4-byte header** that tells the receiver exactly how many bytes to expect. This allows for:
- **Zero-Ambiguity boundaries**: No confusion between multiple messages on the same stream.
- **Memory Efficiency**: The system allocates exactly the right amount of memory for each incoming request.
- **Backpressure Support**: The daemon can signal the CLI to slow down if it becomes overloaded.

### Binary Serialization
We use the **Bitcode** high-efficiency binary serializer instead of text-based formats like JSON.
- **Density**: Serialized messages are up to 10x smaller than equivalent JSON.
- **Speed**: Serialization and deserialization take only a few microseconds.
- **Resilience**: The strictly typed nature of the binary format prevents many common communication errors.

---

## 🔄 Interaction Patterns

### The Action Lifecycle
Every interaction follows a predictable path:
1.  **Request**: The interface packages your command and sends it to the background.
2.  **Processing**: The engine evaluates the task and routes it to the correct handler.
3.  **Result**: The answer is returned instantly, paired with a unique identifier to ensure perfect tracking.
4.  **Error Handling**: Every response provides clear status feedback, from success to specific issues.

---

## 🛡️ Security Model

- **Local-Only**: Communication is restricted to your machine and cannot be accessed from the outside world.
- **Permission Boundary**: Strict access controls ensure that your system state remains private.
- **Integrity Protection**: The structured nature of the messages ensures that malformed or unauthorized data is rejected immediately.
