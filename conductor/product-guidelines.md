# Product Guidelines - OMG

## Prose Style & Tone
- **Technical & Precise:** Documentation and user-facing messages must prioritize accuracy and efficiency, using industry-standard terminology.
- **Friendly & Encouraging:** While maintaining technical depth, we use approachable language to ensure new developers feel empowered to use our advanced tooling.

## Technical Standards
- **Strict Rust Safety:** Zero `unsafe` code is allowed in application logic. Any unavoidable `unsafe` in core abstractions must be strictly isolated and MIRI-verified.
- **Mandatory TDD Protocol:** We follow a rigid Red-Green-Refactor cycle. No feature or bug fix is implemented without a failing test first. We aim for absolute coverage and 100% memory safety.
- **Benchmark-Gated Performance:** Performance is a core feature. All changes to performance-critical paths must include benchmarks to ensure no regressions in our sub-10ms response time guarantees.

## Visual Identity & UX
- **Consistent ANSI Styling:** Use standardized ANSI colors (Green for success, Red for errors, Blue for information) to provide immediate, recognizable feedback.
- **Interactive Dashboards:** Utilize `ratatui` to provide rich, visual insights via `omg dash`, making complex system states easy to understand at a glance.
- **Minimalist by Default:** The CLI follows the principle of least surprise, outputting only what is essential for the task while providing robust verbose modes for deep debugging.
