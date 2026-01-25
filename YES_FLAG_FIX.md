# Fix for `--yes` Flag Not Working with Sudo Elevation

## Problem
The `--yes` flag was not being respected during privilege elevation. When running commands like:
- `omg install package --yes`
- `omg update --yes`
- `omg remove package --yes`

The sudo password prompt would still appear even though `--yes` was specified, breaking automation and CI/CD workflows.

## Root Cause
The privilege elevation code in `src/core/privilege.rs` had two issues:

1. **`elevate_if_needed` function** (lines 130-155): This function re-executes the binary with sudo but didn't check for the `--yes` flag to add the `-n` (non-interactive) flag to sudo.

2. **`run_self_sudo` function** (lines 181-260): This function always tried non-interactive sudo first, then fell back to interactive mode on failure. It had no way to know if the user wanted non-interactive mode only.

## Solution

### 1. Added Global Yes Flag Tracking
In `src/core/privilege.rs`, added thread-local storage to track the `--yes` flag state using atomic operations for thread safety.

### 2. Updated `elevate_if_needed` Function
Modified to check for `--yes` or `-y` in arguments and add sudo's `-n` flag when present.

### 3. Updated `run_self_sudo` Function
Modified to respect the global yes flag and avoid interactive fallback when `--yes` is set.

### 4. Set Yes Flag in Main Binary
In `src/bin/omg.rs`, added code to detect and set the yes flag based on the command.

## Behavior

### With `--yes` Flag
When `--yes` or `-y` is specified:
1. Sudo is invoked with the `-n` flag (non-interactive mode)
2. If password is required, the command fails immediately with a clear error message
3. No fallback to interactive sudo occurs
4. Clear instructions are provided for configuring NOPASSWD in sudoers

### Without `--yes` Flag (Default)
When `--yes` is NOT specified:
1. Sudo is first tried with `-n` flag (non-interactive)
2. If that fails (password required), falls back to interactive sudo
3. User can enter password interactively
4. Preserves backward compatibility with existing behavior

## Files Modified

1. `src/core/privilege.rs` - Added yes flag tracking and updated sudo logic
2. `src/core/mod.rs` - Exported new yes flag functions
3. `src/bin/omg.rs` - Set yes flag based on command arguments
4. `tests/privilege_tests.rs` - Added comprehensive tests

## Testing

Added 7 comprehensive tests in `tests/privilege_tests.rs`:
- `test_yes_flag_prevents_password_prompt` - Verifies flag state tracking
- `test_install_command_parses_yes_flag` - Verifies CLI parsing
- `test_update_command_parses_yes_flag` - Verifies CLI parsing
- `test_remove_command_parses_yes_flag` - Verifies CLI parsing
- `test_yes_flag_with_nopasswd_sudo` - Verifies success with NOPASSWD
- `test_yes_flag_without_nopasswd_fails_clearly` - Verifies clear error messages
- `test_yes_flag_prevents_fallback_to_interactive` - Verifies no interactive fallback

All 28 privilege tests pass successfully.

## Backward Compatibility

This fix maintains full backward compatibility:
- Without `--yes`, behavior is unchanged (tries -n, falls back to interactive)
- With `--yes`, behavior is now correct (non-interactive only)
- No breaking changes to existing workflows
