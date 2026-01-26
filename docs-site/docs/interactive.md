---
title: Interactive Examples
sidebar_position: 3
---

import CLIPlayground from '@site/src/components/CLIPlayground';
import PerformanceBenchmark from '@site/src/components/PerformanceBenchmark';
import CommandComparison from '@site/src/components/CommandComparison';

# Interactive Examples

Try OMG commands right in your browser, see real performance benchmarks, and learn how to migrate from your current tools.

## CLI Playground

Experience OMG without installing anything. Try commands in the interactive terminal below:

<CLIPlayground />

### Available Commands

Try these example commands:
- `omg search firefox` - Search for packages
- `omg use node 20` - Switch Node.js version
- `omg install neovim` - Install a package
- `omg run dev` - Run project tasks
- `omg status` - Check system status
- `omg update` - Check for updates
- `omg help` - Show help

**Pro Tips:**
- Press `Tab` to autocomplete commands
- Use `↑` and `↓` arrow keys to navigate command history
- Commands are simulated with realistic responses

---

<PerformanceBenchmark />

### Why OMG is Faster

OMG achieves its speed through several architectural optimizations:

1. **Parallel Database Queries** - Multiple operations run concurrently
2. **Smart Caching** - Intelligent cache invalidation reduces redundant queries
3. **Native Implementation** - Written in Rust for maximum performance
4. **Daemon Architecture** - Background service keeps metadata warm
5. **Optimized Indexing** - Custom database indices for common queries

These benchmarks represent real-world usage patterns on production systems.

---

<CommandComparison />

### Migration Tips

When switching to OMG:

1. **Start with Search** - Get familiar with `omg search` to find packages
2. **Use One Runtime** - Try `omg use node 20` or similar to test runtime switching
3. **Keep Your Old Tools** - OMG coexists peacefully with existing package managers
4. **Run Tasks** - Use `omg run <task>` to unify your workflow
5. **Check Status** - `omg status` shows everything at a glance

OMG is designed to be incrementally adoptable - use as much or as little as you want.

---

## Next Steps

- **[CLI Reference](/cli)** - Complete command documentation
- **[Quickstart](/quickstart)** - Get OMG installed in 60 seconds
- **[Architecture](/architecture)** - Learn how OMG works under the hood
