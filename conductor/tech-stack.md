# Technology Stack - OMG

## Core Systems
- **Language:** Rust (Edition 2024, v1.92+) - Selected for memory safety, performance, and zero-cost abstractions.
- **Async Runtime:** [Tokio](https://tokio.rs/) (v1.49) - Powering the persistent daemon and high-concurrency operations.
- **CLI Framework:** [Clap](https://docs.rs/clap/latest/clap/) (v4.5) - For robust command-line argument parsing and help generation.
- **TUI Framework:** [Ratatui](https://ratatui.rs/) (v0.28) & [Crossterm](https://docs.rs/crossterm/latest/crossterm/) - For rich, interactive terminal dashboards.

## Data & Communication
- **Database:** [Redb](https://www.redb.org/) (v3.1) - A pure Rust, high-performance embedded key-value store.
- **Serialization:**
    - [Serde](https://serde.rs/) - General serialization/deserialization.
    - [Bitcode](https://docs.rs/bitcode/latest/bitcode/) - High-speed binary format for daemon communication.
    - [Rkyv](https://rkyv.org/) - Zero-copy deserialization for instant package indexing.
- **Communication:** Unix Domain Sockets with custom binary framing for zero-latency client-daemon interaction.

## Package Manager Integration
- **Arch Linux:** `alpm` bindings and official `archlinux/alpm` Rust ecosystem for direct library access.
- **Debian/Ubuntu:** `rust-apt` and `debian-packaging` (pure Rust implementation) for accelerated APT operations.

## Testing & Quality Assurance
- **Unit & Integration:** `cargo test` with a strict TDD protocol.
- **Property-Based Testing:** [Proptest](https://docs.rs/proptest/latest/proptest/) for verifying parsers and CLI logic.
- **Benchmarking:** [Criterion](https://bheisler.github.io/criterion.rs/book/index.html) for performance regression tracking.
- **CLI Verification:** `assert_cmd` and `predicates` for integration testing.
