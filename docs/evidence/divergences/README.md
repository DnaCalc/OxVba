# Divergence Evidence Index

Current records:

- `DIV-0001` — `If ... Then ... End If` unsupported (closed in `mvp-controlflow-v2`)
- `DIV-0002` — `For ... Next` unsupported (closed in `mvp-controlflow-v2`)
- `DIV-0003` — `Implements` baseline divergence closed for current deterministic subset; residual multi-interface oracle uncertainty remains in `CCT-040`/`ODG-038` (closed)
- `DIV-0004` — `WithEvents`/`RaiseEvent` runtime dispatch ordering and subscription semantics pending (open)
- `DIV-0005` — internal class/object lifetime teardown does not yet reliably trigger `Class_Terminate`; Excel oracle shows the old implementation diverges on teardown-sensitive behavior (open)
