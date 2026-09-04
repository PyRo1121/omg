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
