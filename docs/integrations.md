---
title: Integrations
sidebar_position: 13
description: Integrating OMG with other tools and workflows
---

# Integrations

OMG enhances your existing development workflow by integrating seamlessly with popular tools, shells, IDEs, and CI/CD platforms.

---

## 🔍 Search & Navigation

### fzf (Fuzzy Finder)

**Interactive package selection:**

```bash
# Search and install interactively
omg search | fzf | cut -d' ' -f1 | xargs omg install

# Search AUR interactively
omg search --aur | fzf --preview 'omg info {}' | xargs omg install

# Select runtime version
omg list node --available | fzf | xargs omg use node
```

**Add to shell:**
```bash
# ~/.zshrc or ~/.bashrc
alias omgi='omg search | fzf --preview "omg info {1}" | cut -d" " -f1 | xargs omg install'
alias omgn='omg list node --available | fzf | xargs omg use node'
```

---

### ripgrep (Code Search)

**Find OMG usage in projects:**

```bash
# Find version files in project
rg '\.nvmrc|\.python-version|rust-toolchain' --files

# Find omg.lock files across repositories
rg 'omg.lock' --files ~/projects

# Search for OMG commands in CI files
rg 'omg (install|use|run)' .github/workflows
```

---

### fd (File Finder)

**Locate version files:**

```bash
# Find all .nvmrc files
fd '.nvmrc' ~/projects

# Find all omg.lock files
fd 'omg.lock' --type f

# Find all rust-toolchain files
fd 'rust-toolchain' --extension toml
```

---

## 🐚 Shell Integration

### Zsh

**Enhanced completions:**

```bash
# ~/.zshrc
eval "$(omg hook zsh)"

# Enable fuzzy matching
zstyle ':completion:*' matcher-list 'm:{a-z}={A-Z}' 'r:|[._-]=* r:|=*' 'l:|=* r:|=*'

# Show package descriptions in completions
zstyle ':completion:*:descriptions' format '%F{green}-- %d --%f'
```

**Useful aliases:**
```bash
alias oi='omg install'
alias os='omg search'
alias ou='omg use'
alias or='omg run'
alias oe='omg env'
```

---

### Fish

**Fish completions:**

```fish
# ~/.config/fish/config.fish
omg hook fish | source

# Abbreviations
abbr -a oi omg install
abbr -a os omg search
abbr -a ou omg use
abbr -a or omg run
abbr -a oe omg env
```

---

### Bash

```bash
# ~/.bashrc
eval "$(omg hook bash)"

# Aliases
alias oi='omg install'
alias os='omg search'
alias ou='omg use'
alias or='omg run'
```

---

## 💻 IDE & Editor Integration

### VS Code

**Automatic runtime detection:**

OMG's shell integration works automatically with VS Code's integrated terminal.

**Workspace settings:**
```json
// .vscode/settings.json
{
  "terminal.integrated.env.linux": {
    "OMG_AUTO_SWITCH": "true"
  },
  "files.associations": {
    "omg.lock": "json",
    "rust-toolchain.toml": "toml"
  }
}
```

**Recommended extensions:**
- **TOML Language Support** - Syntax highlighting for `omg.toml`, `rust-toolchain.toml`
- **Better TOML** - Enhanced TOML editing

---

### JetBrains IDEs (IntelliJ, PyCharm, WebStorm)

**Configure runtime:**

1. **Settings → Build, Execution, Deployment → Node.js**
   - Set Node interpreter to: `~/.local/share/omg/versions/node/current/bin/node`

2. **Settings → Project → Python Interpreter**
   - Add interpreter: `~/.local/share/omg/versions/python/current/bin/python3`

3. **Settings → Build, Execution, Deployment → Rust**
   - Toolchain location: `~/.local/share/omg/versions/rust/current`

**Auto-detect on project open:**
```bash
# Add to project's .idea/runConfigurations/
# Use OMG-managed runtimes
```

---

### Neovim

**Integrate with Mason.nvim:**

```lua
-- ~/.config/nvim/lua/plugins/mason.lua
require("mason").setup({
  PATH = "prepend", -- Prefer OMG-installed tools
})

-- Ensure OMG runtimes are used
vim.env.PATH = vim.fn.expand("~/.local/share/omg/versions/node/current/bin") 
  .. ":" .. vim.env.PATH
```

**Automatic runtime switching:**
```lua
-- Auto-detect .nvmrc and switch
vim.api.nvim_create_autocmd("DirChanged", {
  pattern = "*",
  callback = function()
    vim.fn.system("omg env check")
  end,
})
```

---

## 🚀 CI/CD Integration

### GitHub Actions

**Basic workflow:**

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install OMG
        run: curl -fsSL https://pyro1121.com/install.sh | bash
      
      - name: Add OMG to PATH
        run: echo "$HOME/.local/bin" >> $GITHUB_PATH
      
      - name: Verify environment
        run: omg env check
      
      - name: Install dependencies
        run: omg install
      
      - name: Run tests
        run: omg run test
```

**Cache OMG installations:**

```yaml
- name: Cache OMG runtimes
  uses: actions/cache@v3
  with:
    path: ~/.local/share/omg
    key: omg-${{ hashFiles('omg.lock') }}
    restore-keys: omg-

- name: Install OMG runtimes
  run: omg env sync
```

---

### GitLab CI

```yaml
# .gitlab-ci.yml
image: archlinux:latest

before_script:
  - curl -fsSL https://pyro1121.com/install.sh | bash
  - export PATH="$HOME/.local/bin:$PATH"
  - omg env check

build:
  script:
    - omg install
    - omg run build

test:
  script:
    - omg run test
```

---

### Jenkins

```groovy
// Jenkinsfile
pipeline {
  agent any
  
  stages {
    stage('Setup') {
      steps {
        sh 'curl -fsSL https://pyro1121.com/install.sh | bash'
        sh 'export PATH="$HOME/.local/bin:$PATH"'
      }
    }
    
    stage('Build') {
      steps {
        sh 'omg env check'
        sh 'omg run build'
      }
    }
    
    stage('Test') {
      steps {
        sh 'omg run test'
      }
    }
  }
}
```

---

### CircleCI

```yaml
# .circleci/config.yml
version: 2.1

jobs:
  build:
    docker:
      - image: archlinux:latest
    steps:
      - checkout
      - run:
          name: Install OMG
          command: curl -fsSL https://pyro1121.com/install.sh | bash
      - run:
          name: Sync environment
          command: |
            export PATH="$HOME/.local/bin:$PATH"
            omg env sync
      - run:
          name: Build
          command: omg run build
```

---

## 🎨 Shell Prompts

### Starship

**Show current runtime versions:**

```toml
# ~/.config/starship.toml

[custom.omg_node]
command = "omg which node 2>/dev/null | cut -d' ' -f2"
when = "test -f package.json || test -f .nvmrc"
format = "via [$symbol($output)]($style) "
symbol = "⬢ "
style = "bold green"

[custom.omg_python]
command = "omg which python 2>/dev/null | cut -d' ' -f2"
when = "test -f .python-version || test -f pyproject.toml"
format = "via [$symbol($output)]($style) "
symbol = "🐍 "
style = "bold yellow"

[custom.omg_rust]
command = "omg which rust 2>/dev/null | cut -d' ' -f2"
when = "test -f Cargo.toml || test -f rust-toolchain.toml"
format = "via [$symbol($output)]($style) "
symbol = "🦀 "
style = "bold red"
```

---

### Oh My Zsh

**Custom theme segment:**

```bash
# ~/.oh-my-zsh/custom/themes/omg.zsh-theme

# Show active runtime
omg_runtime() {
  if [ -f package.json ]; then
    echo "%{$fg[green]%}⬢ $(omg which node | cut -d' ' -f2)%{$reset_color%}"
  elif [ -f Cargo.toml ]; then
    echo "%{$fg[red]%}🦀 $(omg which rust | cut -d' ' -f2)%{$reset_color%}"
  fi
}

PROMPT='$(omg_runtime) %~ %# '
```

---

## 🐳 Container Integration

### Docker

**Use OMG in Dockerfile:**

```dockerfile
# Dockerfile
FROM archlinux:latest

# Install OMG
RUN curl -fsSL https://pyro1121.com/install.sh | bash

# Copy environment lock
COPY omg.lock /app/

# Sync environment
WORKDIR /app
RUN omg env sync

# Build application
RUN omg run build

CMD ["omg", "run", "start"]
```

**Multi-stage build:**

```dockerfile
# Build stage
FROM archlinux:latest AS builder

RUN curl -fsSL https://pyro1121.com/install.sh | bash

COPY . /app
WORKDIR /app

RUN omg env sync && omg run build

# Production stage
FROM archlinux:latest

COPY --from=builder /app/dist /app

CMD ["/app/start"]
```

---

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  app:
    build: .
    volumes:
      - .:/app
      - omg-cache:/root/.local/share/omg
    environment:
      - OMG_AUTO_SWITCH=true

volumes:
  omg-cache:
```

---

## 🔧 Tool Ecosystem

### tmux

**Auto-switch in tmux sessions:**

```bash
# ~/.tmux.conf
set-option -g update-environment "OMG_AUTO_SWITCH"

# Create new window with OMG environment
bind c new-window -c "#{pane_current_path}" \; send-keys "omg env check" C-m
```

---

### direnv

**Combine with direnv:**

```bash
# .envrc
layout omg

# Or custom
use_omg() {
  eval "$(omg hook bash)"
}
```

---

### asdf

**Migrate from asdf:**

```bash
# Migrate .tool-versions
omg migrate from-asdf

# Or use .tool-versions directly (OMG reads it)
cat .tool-versions
# node 20.10.0
# python 3.12.0
# rust stable

omg env check  # Auto-installs all versions
```

---

## 📦 Package Manager Combinations

### Use with yay

**OMG for search, yay for install:**

```bash
# Fast search with OMG
omg search firefox  # 12-24x faster

# Install with yay (if you prefer)
yay -S firefox
```

**Why combine?**
- OMG: Lightning-fast search (6ms vs 133ms)
- yay: Feature-complete installer (VCS packages, etc.)

---

### Use with Homebrew (macOS)

```bash
# Search with OMG
omg search ripgrep

# Install with brew
brew install ripgrep

# Or use OMG's runtime management
omg use node 20
```

---

## 🎯 Workflow Examples

### Full-Stack Development

```bash
# Project setup
cd my-app
echo "20.10.0" > .nvmrc
echo "3.12.0" > .python-version
echo "stable" > rust-toolchain

# OMG auto-detects all
cd .  # Triggers auto-switch
node --version   # 20.10.0
python --version # 3.12.0
rustc --version  # stable

# Lock for team
omg env capture
git add omg.lock

# Run dev server
omg run dev  # Auto-detects package.json scripts
```

---

### Team Onboarding

**New developer setup:**

```bash
# Clone repo
git clone https://github.com/team/project
cd project

# Install OMG
curl -fsSL https://pyro1121.com/install.sh | bash

# Sync entire environment (runtimes + packages)
omg env sync

# Start coding
omg run dev
```

**One command, entire environment synced.**

---

### CI/CD Pipeline

```yaml
# .github/workflows/deploy.yml
name: Deploy

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install OMG
        run: curl -fsSL https://pyro1121.com/install.sh | bash
      
      - name: Sync environment
        run: omg env sync
      
      - name: Build
        run: omg run build
      
      - name: Deploy
        run: omg run deploy
```

---

## 🔗 Integration Tips

### Performance

- **Cache runtimes in CI** - Use `actions/cache` to avoid re-downloading
- **Use omg.lock** - Pin exact versions for reproducibility
- **Enable daemon locally** - Faster repeated operations
- **Disable daemon in CI** - Simpler, more reliable

### Best Practices

- **Commit omg.lock** - Share exact environment with team
- **Use version files** - Auto-detection for seamless switching
- **Test in CI** - Use `omg env check` to verify lock files
- **Document integrations** - Add to your project README

### Troubleshooting

**IDE not using OMG runtime:**
```bash
# Restart IDE after installing runtime
# Or explicitly set interpreter path:
~/.local/share/omg/versions/node/current/bin/node
```

**CI cache issues:**
```yaml
# Clear cache and rebuild
- name: Clear OMG cache
  run: rm -rf ~/.local/share/omg
```

---

## 📚 See Also

- [Shell Integration](shell-integration.md) - Detailed shell setup
- [CI/CD Best Practices](ci-cd-best-practices-2025.md) - Complete CI/CD guide
- [Configuration](configuration.md) - OMG configuration options
- [Team Sync](team.md) - Environment locking and sharing
- [Troubleshooting](troubleshooting.md) - Common integration issues
