# CLI Reference

This page provides a structured overview of OMG commands. For full flags, run `omg <command> --help`.

## Package Management
```bash
omg search <query>           # search official repos + AUR
omg install <pkg...>         # install packages with security grading
omg remove <pkg...>          # remove packages
omg update                   # update official + AUR packages
omg info <pkg>               # detailed package info
omg clean                    # clear caches and orphans
omg explicit                 # list explicitly installed packages
omg sync                     # sync package databases
```

### Interactive Search
```bash
omg search vim -i
```
Interactive mode allows quick selection for install.

## Runtime Management
```bash
omg use <runtime> [version]  # install + activate runtime version
omg list [runtime] --available
omg which <runtime>
```
Supported runtimes: **node, bun, python, go, rust, ruby, java**.

## Shell Integration
```bash
omg hook zsh
omg hook bash
omg hook fish

omg completions zsh
omg completions bash
omg completions fish
```
Install hooks once, then restart your shell to activate PATH management.

## System & Security
```bash
omg status
omg doctor
omg audit
```
`omg audit` requires the daemon for fast metadata access.

## Workflow Helpers
```bash
omg run <task> [-- <args...>]
omg new <stack> <name>
omg tool <install|list|remove>
```

## Team Sync & History
```bash
omg env <capture|check|share|sync>
omg history
omg rollback [id]
```

## Common Patterns
- **Auto-detection**: runtime versions are discovered by scanning parent directories.
- **Daemon acceleration**: searches and info are cached when `omgd` is running.
