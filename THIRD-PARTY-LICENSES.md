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

## Runtime Dependencies

The following Rust crates are used as dependencies during compilation:

- **tokio** (MIT License)
- **serde** (MIT OR Apache-2.0)
- **anyhow** (MIT OR Apache-2.0)
- **reqwest** (MIT OR Apache-2.0)
- And others listed in Cargo.toml

For a complete list of Rust dependencies and their licenses, run:
```bash
cargo license
```

---

## License Compatibility

OMG is licensed under AGPL-3.0, which is compatible with:
- MIT License (permissive, allows AGPL integration)
- Apache-2.0 License (permissive, allows AGPL integration)

The AGPL-3.0 license requires that the complete source code, including modifications,
be made available to users interacting with the software over a network.

---

*Last Updated: January 25, 2026*
