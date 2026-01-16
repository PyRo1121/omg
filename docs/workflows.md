# Workflows

This guide highlights high-value workflows that combine the CLI features into repeatable patterns.

## Environment Lockfiles
```bash
omg env capture
omg env check
omg env share
omg env sync <gist-url>
```
Use `omg.lock` to share runtime and explicit package state across machines.

## Task Runner
```bash
omg run build
omg run test -- --watch
```
OMG auto-detects common project files and activates the correct runtime before running.
Supported task sources include `package.json`, `deno.json`, `Cargo.toml`, `Makefile`, `Taskfile.yml`,
`pyproject.toml` (Poetry), `Pipfile`, `composer.json`, `pom.xml`, and `build.gradle`.
If a task name is unknown, OMG falls back to invoking the command directly with runtime-aware PATH.

## Tool Management
```bash
omg tool install ripgrep
omg tool list
omg tool remove ripgrep
```
Tools are installed into an isolated data dir and linked into the managed PATH.

## Project Scaffolding
```bash
omg new rust my-cli
omg new react my-app
omg new node api-server
```
Scaffolding locks runtime versions immediately for predictable builds.

## Daemon Usage
```bash
omg daemon
# or
omgd --foreground
```
Keep the daemon running for maximum search and info performance.
