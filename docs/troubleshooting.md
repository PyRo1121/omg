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
