# Third-Party Licenses

This document contains the licenses for third-party software components used by OMG.

## Table of Contents

- [mise](#mise)

---

## mise

**Project:** mise - The front-end to your dev env
**Repository:** https://github.com/jdx/mise
**License:** MIT License
**Usage:** OMG bundles mise to provide runtime version management for 100+ languages

### MIT License

```
MIT License

Copyright (c) 2025 Jeff Dickey

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## BSD-3-Clause Licensed Dependencies

The following cryptography dependencies use the BSD-3-Clause license:

### curve25519-dalek
Copyright (c) 2016-2021 Isis Agora Lovecruft, Henry de Valence. All rights reserved.

### ed25519-dalek
Copyright (c) 2017-2021 isis agora lovecruft. All rights reserved.

### x25519-dalek
Copyright (c) 2017-2021 isis agora lovecruft, Henry de Valence. All rights reserved.

### subtle
Copyright (c) 2016-2018 Isis Agora Lovecruft, Henry de Valence. All rights reserved.

### instant
Copyright (c) 2019 sebcrozet. All rights reserved.

**BSD-3-Clause License:**
```
Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

---

## GPL-3.0 Licensed Dependencies (Optional Features)

### alpm & alpm-sys
**License:** GPL-3.0
**Feature:** `arch` (Arch Linux package manager integration)
**Note:** GPL-3.0 is compatible with AGPL-3.0 per Section 13 of the AGPL.

Used for direct integration with libalpm (Arch Linux Package Manager).

---

## LGPL-2.0-or-later Licensed Dependencies (Optional Features)

### sequoia-openpgp
**License:** LGPL-2.0-or-later
**Feature:** `pgp` (PGP signature verification)
**Note:** LGPL allows library usage in AGPL-3.0 projects.

### buffered-reader
**License:** LGPL-2.0-or-later
**Dependency of:** sequoia-openpgp

OpenPGP implementation in Rust. Used as a library dependency without modification.

---

## ISC Licensed Dependencies

The following dependencies use the ISC license (functionally equivalent to MIT):

- **aws-lc-rs** - AWS cryptographic library for Rust
- **inotify** - Linux filesystem event monitoring
- **inotify-sys** - Low-level inotify bindings
- **rustls-webpki** - WebPKI X.509 certificate validation
- **untrusted** - Safe parsing of untrusted inputs

ISC License is very permissive and compatible with AGPL-3.0.

---

## Runtime Dependencies (MIT/Apache-2.0)

The vast majority of Rust crates used are dual-licensed under MIT OR Apache-2.0:

**Core Dependencies:**
- **tokio** (MIT)
- **serde** (MIT OR Apache-2.0)
- **anyhow** (MIT OR Apache-2.0)
- **reqwest** (MIT OR Apache-2.0)
- **clap** (MIT OR Apache-2.0)
- **rayon** (MIT OR Apache-2.0)
- **redb** (MIT OR Apache-2.0)
- **tracing** (MIT)
- And 500+ others listed in Cargo.toml/Cargo.lock

For a complete list of all Rust dependencies and their licenses, run:
```bash
cargo tree --format "{p} {l}"
```

Or for a summary by license type:
```bash
cargo install cargo-license
cargo license
```

---

## License Compatibility

OMG is licensed under AGPL-3.0-or-later, which is compatible with:

| License | Compatible | Notes |
|---------|-----------|-------|
| MIT | ✅ Yes | Permissive, allows AGPL integration |
| Apache-2.0 | ✅ Yes | Permissive with patent grant, allows AGPL integration |
| BSD-2-Clause | ✅ Yes | Permissive |
| BSD-3-Clause | ✅ Yes | Permissive with non-endorsement clause |
| ISC | ✅ Yes | Equivalent to MIT |
| Unlicense | ✅ Yes | Public domain equivalent |
| CC0-1.0 | ✅ Yes | Public domain |
| MPL-2.0 | ✅ Yes | File-level copyleft |
| LGPL-2.0+ | ✅ Yes | Library linking allowed |
| GPL-3.0 | ✅ Yes | Same license family, per AGPL Section 13 |

**Important Notes:**

1. **Apache-2.0 + Commercial Use:** Apache-2.0 has NO restrictions on commercial use. You can monetize AGPL-3.0 software that includes Apache-2.0 dependencies without any issues.

2. **AGPL-3.0 Network Requirement:** The only AGPL-3.0 requirement is that users interacting with modified versions over a network must be able to access the source code (Section 13).

3. **Patent Grant:** Apache-2.0 dependencies provide explicit patent protection, which flows through to this AGPL-3.0 project.

---

*Last Updated: January 25, 2026*
*License Audit Completed: January 25, 2026*
