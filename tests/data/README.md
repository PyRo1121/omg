# Fedora RPM database fixture

`fedora-publicsuffix.rpmhdr` is the unmodified `Packages.blob` for
`publicsuffix-list-dafsa` from the image
`fedora:latest@sha256:6c75d5bf57cb0fa5aa4b92c6a83c86c791644496d9ac230de7711f5b8ec3b898`.
The database is `/usr/lib/sysimage/rpm/rpmdb.sqlite`.

Select the record with:

```sql
SELECT blob FROM Packages JOIN Name USING(hnum)
WHERE Name.key = 'publicsuffix-list-dafsa';
```

The 5676-byte fixture has SHA-256
`b8199f16ec4695266286e869b4125059073bf7c7f01ebced2bf2461b4b669deb`.
It is a database header, not a complete RPM archive or executable payload.

Native `rpm -q publicsuffix-list-dafsa --qf '%{NAME}\n%{VERSION}\n%{RELEASE}\n%{SUMMARY}\n'`
inside that image returned:

```text
publicsuffix-list-dafsa
20260116
1.fc44
Cross-vendor public domain suffix database in DAFSA form
```

Unlike the earlier synthetic fixtures, this record starts with the two big-endian
entry/data lengths and has no archive magic prefix. The integration test inserts
these exact bytes into SQLite and reads them through the production database reader.

## Native DNF upgrade records

`fedora-installed.tsv` and `fedora-upgrades.tsv` were captured from the same pinned
Fedora image using `dnf repoquery --installed --latest-limit=1` and
`dnf repoquery --upgrades --latest-limit=1`. The query format contains actual tab
and newline characters between `name`, `arch`, `evr`, and `repoid` fields.

The capture contains 146 installed records and 58 upgrade candidates. Repository
contents can change; these are parser/matching fixtures, not an assertion that a
future Fedora installation must have exactly 58 updates. Matching uses both name
and architecture and preserves native EVR text rather than comparing with semver.

SHA-256 values:

- Installed: `fe05430102cda666217ecf3a1fbd01d91a133086995078a4cf3d5df22467e823`.
- Upgrades: `10ca59439c192f538a7aa50ea340a335e17f68ebcc1fb1ccdcf76842a410a2f3`.
