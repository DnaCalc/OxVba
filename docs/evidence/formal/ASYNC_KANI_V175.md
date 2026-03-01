# ASYNC_KANI_V175.md

- Timestamp (UTC): 2026-03-01T14:39:56Z
- Run name: 175-kani
- Profile: 175
- Status: 
ot-started (deferred)
- Planned command: ./scripts/run-formal.ps1 -ProfileScope mvp-profile-v175 -RequireKani -UseWslKani
- Preferred dispatcher: ./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "175" -DeferredMode cumulative

## Notes
- Strict lane is intentionally deferred to remote Linux capacity.
- Local non-blocking formal lane remains authoritative for profile gating.
