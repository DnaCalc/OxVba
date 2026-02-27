# Formal Extended Todo

Non-blocking formal issues and follow-up items for later ladder profiles.

## Template
- ID:
- Profile:
- Summary:
- Current status (`todo` / `investigating` / `resolved`):
- Reproduction command:
- Suggested next action:

## Active Items
- ID: FTODO-V2-001
  Profile: v2-v3
  Summary: `cargo-kani` is not installed in current environment, so FO-V2-001/002 and FO-V3-001 cannot execute yet.
  Current status (`todo` / `investigating` / `resolved`): todo
  Reproduction command: `cargo kani --version`
  Suggested next action: install `cargo-kani`, then rerun `./scripts/run-formal.ps1` and update manifest/report status.
