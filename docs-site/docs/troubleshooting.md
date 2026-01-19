---
title: Troubleshooting
sidebar_label: Troubleshooting
sidebar_position: 99
description: Common issues and solutions
---

# Troubleshooting

## Daemon Not Running
- Start the daemon: `omg daemon` or `omgd --foreground`.
- Verify the socket path in `config.toml` if customized.

## Security Audit Fails
- Ensure the daemon is running; `omg audit` uses daemon metadata.
- Check network access for vulnerability checks.

## PATH Not Updated
- Install the shell hook (once): `eval "$(omg hook zsh)"`.
- Restart the shell to activate the PATH updates.

## AUR Build Failures
- Verify base deps: `git`, `curl`, `tar`, `sudo`.
- Run `omg doctor` for system health checks.

## Slow Searches
- Confirm the daemon is running.
- Consider clearing caches with `omg clean`.

## TUI Dashboard Issues
- **Dashboard won't start**: Check terminal supports raw mode and alternate screen.
- **Display garbled**: Try resizing terminal or check `$TERM` environment variable.
- **Data not updating**: Press `r` to refresh or restart the daemon.

## History & Rollback Issues
- **Rollback fails**: Ensure old package versions exist in `/var/cache/pacman/pkg/`.
- **History not saved**: Check write permissions on `~/.local/share/omg/`.
- **AUR rollback unsupported**: Currently only official packages can be rolled back.

## Database Issues
- **Cache corruption**: Delete `~/.local/share/omg/cache.redb` and restart daemon.
- **Permission denied**: Ensure data directory is owned by your user.
