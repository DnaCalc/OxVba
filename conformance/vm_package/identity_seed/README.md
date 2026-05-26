# VM Package Identity Seed Fixtures

These fixtures exercise the first executable-semantic-package identity evidence
surface. They are VM-runnable package fixtures, not JIT execution evidence.

Run with:

```powershell
cargo test -p oxvba-vm --test package_identity_fixtures -- --nocapture
```

The test output prints value snapshots plus package digest, bytecode digest,
slot counts, procedure identity fields, per-procedure slot descriptor digests,
and slot descriptor tokens for each fixture.

The VMR-02 rows cover primitive scalar, `String`/`BStr`, declared `Variant`,
and the current VM-runnable UDT field-alias shape. They do not claim nominal
UDT aggregate descriptors; that remains owned by the later UDT descriptor
evidence work.
