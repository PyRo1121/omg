# Pi Mono No-Slop Baseline

Use Pi's official model: minimal core, explicit extensions, explicit skills, observable state.

Rules:
- Research before editing on non-trivial tasks.
- Prefer plans before multi-file changes.
- Keep the core minimal; treat extra workflow tools as add-ons, not assumptions.
- Never use `as any`, `@ts-ignore`, empty catch blocks, or delete tests to make failures disappear.
- Verify every non-trivial change with the narrowest real check available, then broaden if needed.
- Prefer review gates for risky work.
- Keep memory and instructions in files you control.

Default stack intent:
- Core: Pi + selected extensions.
- Add-ons: optional review, cleanup, and GitNexus-style safety skills.
- Coordination: external tools only when needed.
