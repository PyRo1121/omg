# OMG on Omarchy

**Package and developer-tool installation with security checks built into the workflow.**

OMG existed before its creator switched to Omarchy. Omarchy became his daily driver, and using it every day made the needs of people arriving from Windows and macOS particularly relevant: finding software, selecting runtimes, and understanding what an installation is allowed to do.

OMG brings those tasks into one CLI. Its case for Omarchy is concrete: apply installation controls consistently so users have fewer security settings to discover and assemble themselves. Omarchy should continue to own operating-system updates, migrations, and recovery.

This is a proposal for evaluation, not an announcement of Omarchy adoption or endorsement.

## What Omarchy already provides

Omarchy already makes Arch approachable through package menus and a coordinated update workflow. Its updater handles system packages and installed AUR packages, together with snapshots and migrations. Direct system upgrades can skip that coordination, so Omarchy explicitly guards them. OMG should preserve this behavior. [Omarchy updates](https://omarchy.org/manual/updates/)

Its development menu already offers many language environments, most managed through mise. OMG therefore needs to earn its place through useful installation policy and integration, rather than merely offering another runtime selector. [Development tools](https://omarchy.org/manual/development-tools/)

Omarchy also supplies an existing security baseline, including disk encryption and a firewall. Its base package selection primarily uses official repositories and its own repository; optional AUR installations introduce a different trust decision. [Security](https://omarchy.org/manual/security/)

The AUR menu makes community software accessible, but community availability is not publisher vetting. Omarchy's manual explicitly explains this distinction. This is where clearer decisions and enforced installation boundaries can help a newcomer. [Other packages](https://omarchy.org/manual/other-packages/)

## Where OMG adds value

The important distinction is between a tool supporting a security option and an installation workflow applying that option for the user. OMG uses existing package tools where appropriate, while adding policy around the operations it manages.

**Evidence status:** the hardening described below is implemented in [PR #399](https://github.com/PyRo1121/omg/pull/399), which was open when this document was researched on September 13, 2026. The implementation reference is [commit cc67ab89](https://github.com/PyRo1121/omg/tree/cc67ab89541f07af7b433bf29d142a78954cd882). These are reviewable capabilities, not a claim that the latest downloadable release contains every change. Check [releases](https://github.com/PyRo1121/omg/releases) for installed-version coverage.

| User task | What OMG adds on its managed path | Why it matters |
| --- | --- | --- |
| Install an npm-distributed CLI | Installs into staging with lifecycle scripts disabled, then runs npm signature verification before activation; scripts require an explicit package-scoped exception. | Installation does not silently grant every dependency an install-script execution opportunity. |
| Install a Python CLI | Uses a dedicated virtual environment and requests wheels only by default. Source distributions require an explicit exception. | Separates tool dependencies and avoids source-build execution on the default path. |
| Install a Cargo CLI | Requests the published lockfile with `--locked` unless explicitly overridden. | Makes dependency selection more constrained; it does not sandbox Rust build scripts. |
| Install a Go CLI | Sets checksum/proxy policy, disables CGO and automatic toolchain downloads by default, and exposes explicit exceptions. | Reduces implicit inputs and surprise compiler/toolchain behavior. |
| Update a managed tool | Stages the replacement, checks managed entrypoints, and supports rollback when activation fails. | A failed replacement should not require the user to reconstruct a previously working installation. |
| Build an AUR package | Reviews and rechecks sources, then uses an offline Bubblewrap build with a private home and cleared environment by default. | Limits build-time access instead of relying entirely on the user recognizing dangerous shell code. |
| Install an AUR build result | Inspects archive paths, metadata, links, hooks, and privileged contents before handing sealed bytes to the elevated transaction. Selected system-integrating packages require matching private rebuilds. | Adds checks between community build output and a privileged installation. |

The managed-tool defaults and exception handling are visible in [`src/cli/tool.rs`](https://github.com/PyRo1121/omg/blob/cc67ab89541f07af7b433bf29d142a78954cd882/src/cli/tool.rs). The [AUR workflow](aur.md) documents its gates, required confirmations, and compatibility opt-ins.

### A concrete npm distinction

For an npm-distributed command-line tool, OMG's managed installer first materializes the dependency tree with `--ignore-scripts`. It then runs `npm audit signatures` and refuses activation if that check fails, unless the user explicitly allows an unverified installation for that package. An approved script phase runs after verification. [Verification ordering commit](https://github.com/PyRo1121/omg/commit/d356fee5)

That is a specific advantage over an install performed without those controls. It builds on npm's capabilities and makes their application part of OMG's workflow. Signature verification authenticates evidence about packages; it does not establish that signed code is harmless.

This applies to `omg tool install`'s managed npm path. Selecting Node through `omg use node` puts its vendor tools on PATH; subsequent direct `npm install` commands are not intercepted or automatically hardened by OMG. Project tasks also execute project code. See [runtime management](runtimes.md).

### Protection without pretending exceptions disappear

Some tools need lifecycle scripts, Python source builds, CGO, private registries, or other settings outside the defaults. OMG makes those exceptions explicit and records installation policy receipts. A stricter default can produce a refusal where an unrestricted command would proceed; useful error messages and documented exceptions are part of the product's value. [Policy receipts](https://github.com/PyRo1121/omg/commit/1fc5d2a8), [visible overrides](https://github.com/PyRo1121/omg/commit/f4a89bd4)

On Linux, managed installer subprocesses use `no_new_privs` to prevent execution from gaining new privileges. This does not itself isolate the filesystem or network. AUR's Bubblewrap policy is a separate control. [Privilege restriction](https://github.com/PyRo1121/omg/commit/b76bf0c7)

## How it should fit into Omarchy

The proposed first integration is optional managed developer-tool installation and evaluation of OMG's AUR build/install path. System upgrade ownership stays with Omarchy.

| Area | Proposed responsibility |
| --- | --- |
| OS upgrades, mirror/channel selection, migrations, snapshots | Omarchy's existing update workflow. Do not substitute `omg update` or bypass Omarchy's upgrade guard. |
| Existing mise-managed environments | Keep working. Evaluate OMG on selected tools first; avoid two shell hooks competing to select the same runtime. |
| Managed developer CLIs | Evaluate OMG's defaults, exceptions, activation behavior, and removal experience on a defined set of tools. |
| AUR installation | Evaluate the complete transaction, including dependency installation and compatibility with Omarchy's selected repository channel. |
| Recovery | Preserve native package tools and Omarchy recovery. OMG tool activation rollback is not a whole-system snapshot. |

These are integration requirements, not a claim that Omarchy-specific compatibility has already been tested. In particular, an Arch backend alone does not establish that every package transaction respects Omarchy's additional update coordination. Omarchy's stable mirror also intentionally trails current Arch packages; AUR dependency availability must be assessed against the selected channel. [Update channels and guards](https://omarchy.org/manual/updates/)

OMG can read runtime pins from `mise.toml`, `.mise.toml`, and supported ecosystem version files. This can reduce the need to rewrite version declarations, but does not imply complete mise configuration or plugin compatibility. [Supported runtime detection](runtimes.md)

## What would justify making it a default?

A default needs evidence about ordinary users' experience as well as security mechanisms. An Omarchy evaluation should demonstrate:

1. Successful installation, update, and removal of a representative set of developer tools and AUR packages on a named Omarchy version and channel.
2. Preservation of Omarchy's update guards, migrations, snapshots, and native recovery workflow.
3. Clear failures for rejected packages and understandable, narrowly scoped exceptions for legitimate incompatibilities.
4. Predictable PATH behavior alongside the existing mise setup, with a straightforward way to undo the integration.
5. Release artifacts containing the evaluated hardening, with reproducible commands and linked test results.

This document does not report those integration tests as completed. OMG's [CI evidence](https://github.com/PyRo1121/omg/actions) and [security timeline](https://getomg.xyz/security/) provide separate implementation and verification history. Generic Arch or QEMU success should not be presented as an Omarchy compatibility result unless the run actually tested that environment.

## Follow the implementation

| State | Meaning |
| --- | --- |
| In review | Implemented changes in an open PR, not yet on main. |
| On main | Merged changes; release inclusion still needs checking. |
| Released | Included in an identified tagged build users can install. |

For the current work, start with [PR #399](https://github.com/PyRo1121/omg/pull/399), the [public security timeline](https://getomg.xyz/security/), and the [security model](security.md). Individual changes include [manager configuration isolation](https://github.com/PyRo1121/omg/commit/7b0749d2), [managed entrypoint hashing](https://github.com/PyRo1121/omg/commit/8bb84cbc), and [activation rollback](https://github.com/PyRo1121/omg/commit/8e44e924).

**The proposal:** give Omarchy users an approachable installation workflow that applies additional controls at the point they need them, with public evidence explaining what those controls do. Evaluate that fit on real Omarchy systems, then use the results to decide whether broader adoption is warranted.
