# Product Definition - OMG

## Initial Concept
OMG is the Fastest Unified Package Manager for Arch Linux + All Language Runtimes, designed to stop the switching between multiple system and language package managers.

## Target Audience & Business Model
OMG serves a broad spectrum of users through a tiered value proposition:
- **Individual Developers (Free Tier):** Access to core features (Unified CLI, performance, basic runtime management) equivalent to existing open-source tools.
- **Teams & SREs (Team/Enterprise Tiers):** Licensed features including fleet management, standardized environment locks, advanced audit logs, and enterprise security compliance.
- **Open Source Contributors:** A seamless experience for managing diverse technology stacks without manual tool configuration.

## Key Features & Success Metrics
- **Extreme Performance:** Sub-10ms package queries via a persistent daemon and in-memory indexing (now fully supporting both Arch and Debian/Ubuntu).
- **Universal Unification:** One syntax to rule them all—abstracting over system package managers (Arch/Debian) and language runtimes.
- **Enterprise-Grade Security:** Built-in SLSA, SBOM, and vulnerability scanning, gated by license tiers for professional use.
- **Seamless Collaboration:** Environment synchronization via `omg.lock` to eliminate "works on my machine" issues across teams.
- **Intelligent Task Runner:** Context-aware `omg run` that automatically detects project manifests (npm, cargo, make) and executes with the correct versions.

## UX Philosophy
- **Speed First:** Designed so "fingers move faster than the tool responds."
- **Interactive TUI:** Leveraging `ratatui` for rich dashboards (`omg dash`) and complex conflict resolution.
- **Information Transparency:** Clear, informative feedback for long-running operations while maintaining a minimalist aesthetic for routine tasks.

## Development Methodology
- **Strict TDD Protocol:** We operate as a solo coding team with a rigid Test-Driven Development requirement. No feature is implemented without a failing test first.
- **Zero Unsafe:** "Absolute everything" must be tested. We aim for 100% memory safety with zero `unsafe` blocks in application logic.
- **Performance Regression Testing:** Benchmarks are required for every hot-path change to ensure our speed guarantees are never compromised.
