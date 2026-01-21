---
title: Fleet Management
sidebar_position: 45
description: Manage multiple machines and enforce compliance
---

# Fleet Management

**Enterprise Fleet Control**

OMG provides built-in fleet management capabilities, allowing organizations to monitor compliance, enforce policies, and manage drift across hundreds or thousands of machines.

:::info Enterprise Feature
This feature requires an Enterprise license.
:::

## 📊 Fleet Status

Get a real-time overview of your entire fleet's health.

```bash
omg fleet status
```

This command displays:
- **Total Machines**: Count of active nodes.
- **Health Score**: Overall compliance percentage.
- **Drift Analysis**: Machines that have deviated from the baseline.
- **Team Breakdown**: Compliance stats per team (Frontend, Backend, etc.).

### Verbose Output

For detailed machine-level data:

```bash
omg fleet status --verbose
```

## 🚀 Pushing Configurations

Push configuration updates, policy changes, or immediate remediations to specific teams or the entire fleet.

```bash
# Push to all machines
omg fleet push

# Push to a specific team
omg fleet push --team frontend --message "Update node version"
```

The push command:
1. Prepares the configuration payload.
2. Authenticates with the fleet server.
3. Broadcasts the update to connected agents.
4. Reports success/failure rates.

## 🔧 Automated Remediation

Fix configuration drift automatically.

```bash
# Preview changes (Dry Run)
omg fleet remediate --dry-run

# Apply fixes
omg fleet remediate --confirm
```

Remediation handles:
- **Package Updates**: Installing missing packages or updating versions.
- **Runtime Versions**: Switching languages to required versions.
- **Policy Enforcement**: Re-applying security policies.

## Integration

The fleet agent runs as part of the `omgd` background service, checking in with the central control plane periodically. It uses minimal resources and respects network bandwidth limits.
