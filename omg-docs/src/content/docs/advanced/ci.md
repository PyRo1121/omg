---
title: "CI/CD Integration"
description: "Automated pipeline configuration and validation"
---

# CI/CD Integration

**Automate Your Workflows**

OMG includes tools to automatically generate and validate CI/CD configurations for popular providers.

## ⚡ Quick Setup

Generate a starter configuration for your CI provider.

```bash
# GitHub Actions
omg ci init github

# GitLab CI
omg ci init gitlab

# CircleCI
omg ci init circleci
```

This creates the necessary workflow files (e.g., `.github/workflows/ci.yml`) with best practices pre-configured:
- Caching strategies
- Environment synchronization
- Build and test steps

## 🔍 Environment Validation

Ensure your CI environment matches your local development environment exactly.

```bash
omg ci validate
```

This command:
1. Checks for `omg.lock`.
2. Captures the current CI environment state.
3. Compares it against the lockfile.
4. Fails the build if drift is detected.

Add this step to your pipeline to prevent "it works on my machine" issues in production.

## 💾 Caching Strategy

Performance is critical in CI. OMG provides helper commands to identify what paths should be cached.

```bash
omg ci cache
```

Outputs recommended cache paths for:
- OMG data (`~/.local/share/omg`)
- Runtime versions
- Package manager caches (Cargo, npm, pacman)

### GitHub Actions Example

```yaml
- name: Cache OMG data
  uses: actions/cache@v4
  with:
    path: |
      ~/.local/share/omg
      ~/.cargo/registry
    key: omg-${{ runner.os }}-${{ hashFiles('omg.lock') }}
```
