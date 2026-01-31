# Fix OpenClaw Twitter Integration

## TL;DR

> **Quick Summary**: Bird CLI requires `AUTH_TOKEN` and `CT0` environment variables but openclaw stores them in config. Fix by updating the Twitter skill to auto-export credentials before each bird command.
> 
> **Deliverables**:
> - Updated Twitter SKILL.md with credential export commands
> - Bird wrapper script at `~/.openclaw/bin/bird-wrapped`
> - Working Twitter search, read, and post functionality
> 
> **Estimated Effort**: Quick (15-20 minutes)
> **Parallel Execution**: NO - sequential
> **Critical Path**: Task 1 → Task 2 → Task 3

---

## Context

### Original Request
User's openclaw Twitter integration stopped working. Bird CLI can't find credentials.

### Research Findings
**Credentials are stored correctly** in:
- `~/.openclaw/.env`: AUTH_TOKEN and CT0 as plain env vars
- `~/.openclaw/openclaw.json`: env.BIRD_AUTH_TOKEN and env.BIRD_CT0

**Root Cause**: Bird CLI searches for credentials in this order:
1. Browser cookies (Safari/Chrome/Firefox)
2. `AUTH_TOKEN` and `CT0` environment variables
3. Command-line flags `--auth-token` and `--ct0`

The credentials exist but aren't **exported** when bird runs, so bird can't see them.

**Proof it works with manual export**:
```bash
export AUTH_TOKEN="3f374960d44b521034e7c200a7c536a775f05fb7"
export CT0="4865a9a7de382babd1257d8557d6f634eac879ab61f9d4b434b84749b4bbbb3e64309cd83629fa5659fd45caa016f5fbad433805e17627f9dec852f63cdaad332b56a2e0b33a3d77293b824ad3cdcbf4"
bird search "AI agents" -n 2
# ✅ Returns results successfully
```

---

## Work Objectives

### Core Objective
Make Twitter skill work automatically without requiring manual credential export.

### Concrete Deliverables
- Updated `~/.openclaw/skills/twitter/SKILL.md` with proper credential handling
- Bird wrapper script at `~/.openclaw/bin/bird-wrapped` 
- All bird commands work seamlessly through openclaw agent

### Definition of Done
- [ ] `bird search "test" -n 1` works without manual export
- [ ] Openclaw agent can execute Twitter commands
- [ ] Twitter skill documentation is accurate

### Must Have
- Automatic credential injection for all bird commands
- No security regressions (credentials not leaked to logs)

### Must NOT Have (Guardrails)
- Don't hardcode credentials in files (use openclaw config)
- Don't break existing openclaw config structure
- Don't modify bird CLI source code

---

## Verification Strategy

### Manual Verification Only (NO User Intervention)

> **CRITICAL PRINCIPLE: ZERO USER INTERVENTION**
>
> All verification MUST be automated and executable by the agent.

Each TODO includes EXECUTABLE verification procedures:

**For Twitter Skill Updates:**
```bash
# Agent executes via Bash tool:
1. cat ~/.openclaw/skills/twitter/SKILL.md | grep "export AUTH_TOKEN"
2. Expected: Commands now include credential export prefix
```

**For Wrapper Script:**
```bash
# Agent tests wrapper:
1. ~/.openclaw/bin/bird-wrapped search "test" -n 1
2. Expected: Returns 1 tweet result
3. Exit code: 0
```

**For Integration Test:**
```bash
# Agent runs full workflow:
1. bird-wrapped search "AI" -n 2
2. Assert: Output contains "📅" (date emoji in bird output)
3. Assert: Exit code 0
4. Screenshot: Not applicable (CLI tool)
```

---

## TODOs

- [ ] 1. Update Twitter Skill Documentation

  **What to do**:
  - Open `~/.openclaw/skills/twitter/SKILL.md`
  - Add note at top: "IMPORTANT: Commands automatically use credentials from openclaw config"
  - Update all bird command examples to prefix with credential export:
    ```bash
    export AUTH_TOKEN="$BIRD_AUTH_TOKEN" CT0="$BIRD_CT0" && bird search "query"
    ```
  - Document that openclaw agent will handle this automatically

  **Must NOT do**:
  - Don't hardcode actual credential values in examples
  - Don't remove existing command documentation
  - Don't change bird CLI installation path

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Reason**: Simple text file update, straightforward task
  - **Skills**: None needed (just file editing)

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 2 (wrapper script references updated docs)
  - **Blocked By**: None

  **References**:
  
  **Current File**:
  - `~/.openclaw/skills/twitter/SKILL.md` - Twitter skill documentation to update

  **Credential Locations** (for reference only, don't modify):
  - `~/.openclaw/.env` - Contains AUTH_TOKEN and CT0
  - `~/.openclaw/openclaw.json:env.BIRD_AUTH_TOKEN` - Stored credential
  - `~/.openclaw/openclaw.json:env.BIRD_CT0` - Stored credential

  **Bird CLI Documentation**:
  - `/home/pyro1121/.cache/.bun/bin/bird --help` - Command reference
  - Bird expects: `AUTH_TOKEN` and `CT0` as environment variables

  **Acceptance Criteria**:

  **Automated Verification**:
  ```bash
  # Agent runs via Bash:
  grep -q "export AUTH_TOKEN" ~/.openclaw/skills/twitter/SKILL.md
  # Assert: Exit code 0 (pattern found)
  
  grep -q "BIRD_AUTH_TOKEN" ~/.openclaw/skills/twitter/SKILL.md  
  # Assert: Exit code 0 (references config vars)
  
  wc -l ~/.openclaw/skills/twitter/SKILL.md
  # Assert: Line count increased (documentation added)
  ```

  **Evidence to Capture**:
  - Terminal output showing grep matches
  - Diff of file changes

  **Commit**: YES
  - Message: `fix(openclaw): update Twitter skill docs with credential export`
  - Files: `~/.openclaw/skills/twitter/SKILL.md`
  - Pre-commit: None (documentation change)

---

- [ ] 2. Create Bird Wrapper Script

  **What to do**:
  - Create directory: `mkdir -p ~/.openclaw/bin`
  - Create script: `~/.openclaw/bin/bird-wrapped`
  - Script should:
    1. Source credentials from openclaw config using `openclaw config get`
    2. Export AUTH_TOKEN and CT0
    3. Execute actual bird CLI with all passed arguments
  - Make executable: `chmod +x ~/.openclaw/bin/bird-wrapped`
  - Script template:
    ```bash
    #!/bin/bash
    export AUTH_TOKEN=$(openclaw config get env.BIRD_AUTH_TOKEN)
    export CT0=$(openclaw config get env.BIRD_CT0)
    exec /home/pyro1121/.cache/.bun/bin/bird "$@"
    ```

  **Must NOT do**:
  - Don't modify the actual bird binary
  - Don't hardcode credential values in wrapper
  - Don't change bird's behavior, only add credential injection

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Reason**: Small bash script, single file creation
  - **Skills**: None needed

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential (after Task 1)
  - **Blocks**: Task 3 (testing needs wrapper)
  - **Blocked By**: Task 1 (documentation should be updated first)

  **References**:

  **Bird Binary Location**:
  - `/home/pyro1121/.cache/.bun/bin/bird` - Actual bird CLI to wrap

  **Openclaw Config Commands**:
  - `openclaw config get env.BIRD_AUTH_TOKEN` - Retrieve stored auth token
  - `openclaw config get env.BIRD_CT0` - Retrieve stored ct0 cookie

  **Similar Wrapper Patterns** (if needed):
  - Many CLIs use wrapper scripts to inject environment
  - Standard pattern: source vars → export → exec original binary

  **Acceptance Criteria**:

  **Automated Verification**:
  ```bash
  # Agent tests wrapper creation:
  test -f ~/.openclaw/bin/bird-wrapped
  # Assert: Exit code 0 (file exists)
  
  test -x ~/.openclaw/bin/bird-wrapped
  # Assert: Exit code 0 (file is executable)
  
  head -1 ~/.openclaw/bin/bird-wrapped
  # Assert: Output is "#!/bin/bash"
  
  # Test wrapper functionality:
  ~/.openclaw/bin/bird-wrapped --help
  # Assert: Exit code 0
  # Assert: Output contains "bird" help text
  ```

  **Evidence to Capture**:
  - Script file content
  - Execution test output

  **Commit**: YES
  - Message: `feat(openclaw): add bird wrapper for credential injection`
  - Files: `~/.openclaw/bin/bird-wrapped`
  - Pre-commit: `test -x ~/.openclaw/bin/bird-wrapped`

---

- [ ] 3. Test Twitter Functionality

  **What to do**:
  - Test wrapper script with live Twitter API:
    1. Search: `~/.openclaw/bin/bird-wrapped search "test" -n 1`
    2. Verify output format matches expected bird CLI output
  - Test through original bird path (should fail, confirming wrapper is needed):
    1. `bird search "test" -n 1` (expected to fail with credential error)
  - Document which path works in test output

  **Must NOT do**:
  - Don't spam Twitter API with excessive requests (rate limits)
  - Don't post test tweets (read-only testing)
  - Don't share credentials in test output

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Reason**: Simple command execution and output verification
  - **Skills**: None needed

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential (after Task 2)
  - **Blocks**: Task 4 (agent testing needs basic functionality verified)
  - **Blocked By**: Task 2 (wrapper must exist)

  **References**:

  **Bird CLI Command Reference**:
  - `bird search <query> -n <limit>` - Search Twitter
  - `bird read <tweet-url>` - Read specific tweet
  - Expected output format: tweet text, date (📅), link (🔗)

  **Test Queries** (safe, won't trigger rate limits):
  - `bird-wrapped search "test" -n 1` - Single result test
  - `bird-wrapped search "AI" -n 2` - Multi-result test

  **Expected Success Indicators**:
  - Output contains tweet text
  - Output contains date emoji 📅
  - Output contains link emoji 🔗
  - Exit code 0

  **Acceptance Criteria**:

  **Automated Verification**:
  ```bash
  # Agent tests wrapper:
  ~/.openclaw/bin/bird-wrapped search "test" -n 1 > /tmp/bird-test.txt
  # Assert: Exit code 0
  
  grep -q "📅" /tmp/bird-test.txt
  # Assert: Exit code 0 (date emoji found)
  
  grep -q "https://x.com" /tmp/bird-test.txt
  # Assert: Exit code 0 (tweet URL found)
  
  # Verify original bird still fails (confirms wrapper is needed):
  bird search "test" -n 1 2>&1 | grep -q "Missing required credentials"
  # Assert: Exit code 0 (error message found as expected)
  ```

  **Evidence to Capture**:
  - Test output file `/tmp/bird-test.txt`
  - Exit codes from both commands

  **Commit**: NO (testing only)

---

- [ ] 4. Update Twitter Skill to Use Wrapper

  **What to do**:
  - Update `~/.openclaw/skills/twitter/SKILL.md`:
    1. Change all `bird` commands to `bird-wrapped`
    2. Or add instructions for openclaw agent to use wrapper path
    3. Add note: "When calling bird from scripts, use bird-wrapped or export credentials first"
  - Alternative approach (if simpler):
    - Create symlink: `ln -sf ~/.openclaw/bin/bird-wrapped ~/.openclaw/bin/bird`
    - Update PATH in openclaw config to prioritize `~/.openclaw/bin`

  **Must NOT do**:
  - Don't break existing bird CLI for manual use
  - Don't modify system-wide PATH permanently

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Reason**: Documentation update or simple symlink
  - **Skills**: None needed

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential (after Task 3)
  - **Blocks**: None (final task)
  - **Blocked By**: Task 3 (must verify wrapper works first)

  **References**:

  **Skill Documentation**:
  - `~/.openclaw/skills/twitter/SKILL.md` - To update with wrapper usage

  **Openclaw Execution Context**:
  - When openclaw agent executes skills, it runs in a controlled environment
  - Skills can specify custom PATHs or command prefixes
  - Check if skill metadata supports command override

  **Acceptance Criteria**:

  **Automated Verification**:
  ```bash
  # Agent verifies documentation update:
  grep -q "bird-wrapped" ~/.openclaw/skills/twitter/SKILL.md
  # Assert: Exit code 0 (wrapper referenced)
  
  # Or verify symlink approach:
  test -L ~/.openclaw/bin/bird && readlink ~/.openclaw/bin/bird | grep -q "bird-wrapped"
  # Assert: Exit code 0 (symlink points to wrapper)
  ```

  **Integration Test**:
  ```bash
  # Test through openclaw (if agent command syntax allows):
  # This would test end-to-end integration
  # Format TBD based on openclaw agent invocation syntax
  ```

  **Evidence to Capture**:
  - Updated skill documentation or symlink listing
  - Integration test output

  **Commit**: YES
  - Message: `fix(openclaw): Twitter skill now uses credential wrapper`
  - Files: `~/.openclaw/skills/twitter/SKILL.md` or symlink
  - Pre-commit: `test -x ~/.openclaw/bin/bird-wrapped`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `fix(openclaw): update Twitter skill docs with credential export` | `~/.openclaw/skills/twitter/SKILL.md` | grep test |
| 2 | `feat(openclaw): add bird wrapper for credential injection` | `~/.openclaw/bin/bird-wrapped` | execution test |
| 4 | `fix(openclaw): Twitter skill now uses credential wrapper` | skill docs or symlink | integration test |

---

## Success Criteria

### Verification Commands
```bash
# Twitter search works without manual export
~/.openclaw/bin/bird-wrapped search "AI" -n 1
# Expected: Returns 1 tweet with proper formatting

# Skill documentation is updated
grep "bird-wrapped" ~/.openclaw/skills/twitter/SKILL.md
# Expected: Exit code 0
```

### Final Checklist
- [ ] All "Must Have" present (automatic credential injection)
- [ ] All "Must NOT Have" absent (no hardcoded credentials)
- [ ] Twitter commands work through wrapper
- [ ] Documentation accurately reflects new behavior
