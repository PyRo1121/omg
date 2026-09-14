# Mise project workflow integration

Continue the approved native mise compatibility plan after merged PR #403. Work without subagents. Preserve the caller's existing runtime-installation and explicit-task execution boundaries.

## Runtime pins

- Expose one-directory discovery from the shared configuration loader; keep the environment loader's ancestor order and global document bound unchanged.
- Resolve supported native tool pins from each directory's ordered mise documents. Normalize the existing runtime aliases consistently. Preserve supported string/inline-version values and the documented unsupported backend boundary.
- Keep nearest-project pins first and same-directory dedicated version files above mise. Preserve the existing isolation of malformed ancestor pins.
- Add pure module tests and hook integration regressions for local/selected layers, parent/child precedence, aliases, and dedicated pin precedence. Automatic hooks still never import project environment directives.

## Tasks

- Load task declarations through the shared ordered documents, retaining each task's source and configuration root.
- Parse string shorthand, run arrays, dependency-only tasks, dependencies, and working directories into a native model before execution. Reject malformed run entries and unsupported dependency forms explicitly.
- Build a deterministic dependency plan. Detect cycles and missing dependencies before executing any task, deduplicate shared dependencies, and stop dependent work after failure.
- Execute each task with its own declaring root and environment through the existing runtime/setup and process path. Preserve normal ecosystem task selection and argv boundaries.
- Add parser/planner tests to the portable production-module harness and native shell execution regressions to normal CI.

## Finish

Update the compatibility inventory and migration notes. Run formatting, focused tests, and repository lint settings. Push a prefixed PR, fix any hosted failures, and merge only after full CI, coverage, CodeQL, Docker, and QEMU results pass for its actual head.
