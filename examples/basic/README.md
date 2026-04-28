# Basic Runnable Examples

Small runnable VBA examples for the basic language surface, including one
multi-module project directory. They are designed to run without COM, host
application, wrapper, XLL, or Office prerequisites.

```powershell
./scripts/run-basic-examples.ps1
./scripts/run-basic-examples.ps1 -Backend jit
```

The checker compares each single-file example's semantic `VALUES:` output with
`expected.csv`, then runs the project examples listed in `projects/expected.csv`.
