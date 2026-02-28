param(
    [ValidateSet("Ensure", "ProbeCapacity", "StartDeferred", "Status", "Tail", "FetchArtifacts")]
    [string]$Action = "Status",
    [string]$SshHost = "94.72.99.81",
    [string]$SshUser = "ubuntu",
    [string]$SshKeyPath = "$env:USERPROFILE\.ssh\acfs_ed25519",
    [string]$RemoteBase = "/home/ubuntu/.dnacalc_remote",
    [ValidateSet("cumulative", "exact")]
    [string]$DeferredMode = "cumulative",
    [string]$DeferredVersions = "",
    [int]$DeferredConcurrency = 0,
    [string]$DispatchJobName = "deferred-dispatch",
    [int]$TailLines = 80,
    [string]$Lane = "",
    [string]$LocalArtifactsDir = "temp/async/kani_remote"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command on PATH: $Name"
    }
}

function Invoke-RemoteScript {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ScriptText
    )

    $sshTarget = "$SshUser@$SshHost"
    $remoteCommand = "tr -d '\r' | sed '1s/^\xEF\xBB\xBF//' | bash -s"
    $out = $ScriptText | & ssh -i $SshKeyPath $sshTarget $remoteCommand
    if ($LASTEXITCODE -ne 0) {
        throw "Remote command failed with exit code $LASTEXITCODE"
    }
    return $out
}

function Get-EnsureScript([string]$BaseDir) {
    $template = @'
set -euo pipefail
BASE="__BASE__"
mkdir -p "$BASE"/{bin,logs,state/jobs,state/deferred_lanes,state/deferred_dispatch,artifacts,tmp,home,cargo,rustup,work,tools}

cat > "$BASE/bin/env.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
BASE="__BASE__"
export BASE
export TMPDIR="$BASE/tmp"
export HOME="$BASE/home"
export CARGO_HOME="$BASE/cargo"
export RUSTUP_HOME="$BASE/rustup"
export PATH="$BASE/tools/bin:$BASE/cargo/bin:$PATH"
mkdir -p "$TMPDIR" "$HOME" "$CARGO_HOME" "$RUSTUP_HOME" "$BASE/logs" "$BASE/state/jobs" "$BASE/artifacts" "$BASE/work"
EOF

cat > "$BASE/bin/bootstrap_kani.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
log="$BASE/logs/bootstrap_kani.log"
exec > >(tee -a "$log") 2>&1

echo "[bootstrap] start $(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ ! -x "$CARGO_HOME/bin/rustup" ]]; then
  echo "[bootstrap] installing rustup into $BASE"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$BASE/tmp/rustup-init.sh"
  chmod +x "$BASE/tmp/rustup-init.sh"
  RUSTUP_INIT_SKIP_SUDO_CHECK=yes "$BASE/tmp/rustup-init.sh" -y --no-modify-path --default-toolchain nightly --profile minimal
fi

"$CARGO_HOME/bin/rustup" toolchain install nightly
"$CARGO_HOME/bin/rustup" default nightly

if [[ ! -x "$BASE/tools/bin/cargo-kani" ]]; then
  echo "[bootstrap] installing kani-verifier"
  cargo +nightly install --locked kani-verifier --root "$BASE/tools"
fi

echo "[bootstrap] running cargo-kani setup"
"$BASE/tools/bin/cargo-kani" setup

echo "[bootstrap] cargo-kani version"
"$BASE/tools/bin/cargo-kani" --version

echo "[bootstrap] done $(date -u +%Y-%m-%dT%H:%M:%SZ)"
EOF

cat > "$BASE/bin/run_and_record.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ $# -lt 2 ]]; then
  echo "usage: $0 <job_id> <command...>" >&2
  exit 2
fi
job_id="$1"
shift
job_dir="$BASE/state/jobs/$job_id"
mkdir -p "$job_dir"
log="$job_dir/run.log"
printf '%s\n' "start=$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$job_dir/meta"
set +e
"$@" > "$log" 2>&1
code=$?
set -e
printf '%s\n' "$code" > "$job_dir/exit_code"
printf '%s\n' "end=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$job_dir/meta"
exit "$code"
EOF

cat > "$BASE/bin/start_job.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ $# -lt 2 ]]; then
  echo "usage: $0 <job_name> <command...>" >&2
  exit 2
fi
name="$1"
shift
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
job_id="${stamp}_${name}"
job_dir="$BASE/state/jobs/$job_id"
mkdir -p "$job_dir"
cmd=()
for arg in "$@"; do
  cmd+=("$arg")
done
printf '%s\n' "$job_id" > "$job_dir/job_id"
printf '%s\n' "${cmd[*]}" > "$job_dir/command.txt"
nohup "__BASE__/bin/run_and_record.sh" "$job_id" "${cmd[@]}" > "$job_dir/nohup.log" 2>&1 &
pid=$!
printf '%s\n' "$pid" > "$job_dir/pid"
printf '%s\n' "$job_id"
echo "started $job_id pid=$pid"
EOF

cat > "$BASE/bin/job_status.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ $# -ne 1 ]]; then
  echo "usage: $0 <job_id>" >&2
  exit 2
fi
job_id="$1"
job_dir="$BASE/state/jobs/$job_id"
if [[ ! -d "$job_dir" ]]; then
  echo "missing:$job_id"
  exit 1
fi
pid="$(cat "$job_dir/pid")"
log="$job_dir/run.log"
if kill -0 "$pid" 2>/dev/null; then
  echo "running:$job_id pid=$pid"
  exit 0
fi
if [[ -f "$job_dir/exit_code" ]]; then
  code="$(cat "$job_dir/exit_code")"
else
  code="unknown"
fi
echo "finished:$job_id pid=$pid exit=$code"
if [[ -f "$log" ]]; then
  tail -n 40 "$log"
fi
EOF

cat > "$BASE/bin/list_jobs.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
for d in "$BASE"/state/jobs/*; do
  [[ -d "$d" ]] || continue
  job_id="$(basename "$d")"
  pid="$(cat "$d/pid" 2>/dev/null || echo "-")"
  if [[ "$pid" != "-" ]] && kill -0 "$pid" 2>/dev/null; then
    state="running"
  elif [[ -f "$d/exit_code" ]]; then
    state="finished:$(cat "$d/exit_code")"
  else
    state="unknown"
  fi
  echo "$job_id $state pid=$pid"
done
EOF

cat > "$BASE/bin/fetch_job_artifacts.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ $# -ne 1 ]]; then
  echo "usage: $0 <job_id>" >&2
  exit 2
fi
job_id="$1"
job_dir="$BASE/state/jobs/$job_id"
if [[ ! -d "$job_dir" ]]; then
  echo "missing:$job_id" >&2
  exit 1
fi
out="$BASE/artifacts/${job_id}.tar.gz"
tar -czf "$out" -C "$BASE/state/jobs" "$job_id"
echo "$out"
EOF

cat > "$BASE/bin/probe_capacity.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cpu="$(nproc)"
mem_kib="$(awk '/MemAvailable/ {print $2}' /proc/meminfo)"
mem_gib=$((mem_kib / 1024 / 1024))
by_cpu=$((cpu / 8))
by_mem=$((mem_gib / 24))
if (( by_cpu < 1 )); then by_cpu=1; fi
if (( by_mem < 1 )); then by_mem=1; fi
conc="$by_cpu"
if (( by_mem < conc )); then conc="$by_mem"; fi
if (( conc > 4 )); then conc=4; fi
printf 'cpu=%s\nmem_gib=%s\nrecommended_concurrency=%s\n' "$cpu" "$mem_gib" "$conc"
EOF

cat > "$BASE/bin/sync_repo.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
repo="$BASE/work/OxVba"
url="https://github.com/DnaCalc/OxVba.git"
if [[ ! -d "$repo/.git" ]]; then
  git clone "$url" "$repo" >/dev/null
else
  git -C "$repo" fetch --all --prune >/dev/null
  git -C "$repo" reset --hard origin/master >/dev/null
fi
git -C "$repo" rev-parse HEAD
EOF

cat > "$BASE/bin/run_formal_lane.py" <<'EOF'
#!/usr/bin/env python3
import argparse
import csv
import datetime as dt
import json
import pathlib
import re
import subprocess
import sys

def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

def parse_version(profile: str) -> int:
    m = re.search(r"v(\d+)$", (profile or "").strip())
    if not m:
        return -1
    return int(m.group(1))

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--target-version", type=int, required=True)
    ap.add_argument("--mode", choices=["cumulative", "exact"], default="cumulative")
    ap.add_argument("--filter", choices=["kani", "all"], default="kani")
    ap.add_argument("--obligations", default="docs/evidence/formal/obligations.csv")
    ap.add_argument("--report-dir", required=True)
    args = ap.parse_args()

    repo = pathlib.Path(args.repo).resolve()
    obligations_path = (repo / args.obligations).resolve()
    report_dir = pathlib.Path(args.report_dir).resolve()
    report_dir.mkdir(parents=True, exist_ok=True)

    if not obligations_path.exists():
        raise SystemExit(f"missing obligations file: {obligations_path}")

    selected = []
    with obligations_path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            if (row.get("active") or "").strip().lower() != "true":
                continue
            version = parse_version(row.get("profile", ""))
            if version < 0:
                continue
            if args.mode == "cumulative" and version > args.target_version:
                continue
            if args.mode == "exact" and version != args.target_version:
                continue
            cmd = (row.get("command") or "").strip()
            if args.filter == "kani" and not cmd.lower().startswith("cargo kani"):
                continue
            selected.append(row)

    rows = []
    failures = 0
    for row in selected:
        obligation = row.get("obligation_id", "unknown")
        cmd = (row.get("command") or "").strip()
        log_path = report_dir / f"{obligation}.log"
        started = now_utc()
        proc = subprocess.run(
            ["bash", "-lc", cmd],
            cwd=str(repo),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        ended = now_utc()
        output = proc.stdout if proc.stdout is not None else ""
        log_path.write_text(output, encoding="utf-8")
        status = "pass" if proc.returncode == 0 else "todo"
        if proc.returncode != 0:
            failures += 1
        rows.append(
            {
                "obligation": obligation,
                "profile": row.get("profile", ""),
                "command": cmd,
                "status": status,
                "exit_code": proc.returncode,
                "artifact": row.get("artifact", ""),
                "started_utc": started,
                "ended_utc": ended,
                "log": str(log_path),
            }
        )

    csv_path = report_dir / "formal_lane.csv"
    with csv_path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "obligation",
                "profile",
                "status",
                "exit_code",
                "command",
                "artifact",
                "started_utc",
                "ended_utc",
                "log",
            ],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(row)

    summary = {
        "timestamp_utc": now_utc(),
        "repo": str(repo),
        "target_version": args.target_version,
        "mode": args.mode,
        "filter": args.filter,
        "selected_count": len(rows),
        "failures": failures,
        "status": "pass" if failures == 0 else "fail",
        "report_csv": str(csv_path),
    }
    (report_dir / "formal_lane.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    md_lines = [
        "# Remote Formal Lane",
        "",
        f"- Timestamp (UTC): {summary['timestamp_utc']}",
        f"- Target version: v{args.target_version}",
        f"- Mode: {args.mode}",
        f"- Filter: {args.filter}",
        f"- Selected obligations: {len(rows)}",
        f"- Failures: {failures}",
        f"- Status: {summary['status']}",
        "",
        "| Obligation | Profile | Status | Exit | Command |",
        "|---|---|---|---:|---|",
    ]
    for row in rows:
        md_lines.append(
            f"| {row['obligation']} | {row['profile']} | {row['status']} | {row['exit_code']} | {row['command']} |"
        )
    (report_dir / "formal_lane.md").write_text("\n".join(md_lines) + "\n", encoding="utf-8")

    if len(rows) == 0:
        return 0
    return 0 if failures == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
EOF

cat > "$BASE/bin/run_deferred_lane.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ $# -ne 2 ]]; then
  echo "usage: $0 <profile_version> <mode:cumulative|exact>" >&2
  exit 2
fi
v="$1"
mode="$2"
repo="$BASE/work/OxVba"
lane="v${v}-kani"
lane_dir="$BASE/state/deferred_lanes/$lane"
mkdir -p "$lane_dir"
export CARGO_TARGET_DIR="$BASE/work/targets/$lane"
python3 "$BASE/bin/run_formal_lane.py" --repo "$repo" --target-version "$v" --mode "$mode" --filter kani --report-dir "$lane_dir"
EOF

cat > "$BASE/bin/dispatch_deferred_lanes.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
mkdir -p "$BASE/state/deferred_lanes"

mode="${DEFERRED_MODE:-cumulative}"
if [[ "$mode" != "cumulative" && "$mode" != "exact" ]]; then
  echo "invalid DEFERRED_MODE=$mode" >&2
  exit 2
fi

mapfile -t cap < <("$BASE/bin/probe_capacity.sh")
recommended="$(printf '%s\n' "${cap[@]}" | awk -F= '$1=="recommended_concurrency"{print $2}')"
concurrency="${DEFERRED_CONCURRENCY:-$recommended}"
if [[ -z "$concurrency" || "$concurrency" -lt 1 ]]; then
  concurrency=1
fi

commit="$($BASE/bin/sync_repo.sh)"

if [[ -n "${DEFERRED_VERSIONS:-}" ]]; then
  read -r -a versions <<< "${DEFERRED_VERSIONS}"
else
  versions=(81 82 83 87 88 89 90 91 93 94 95 96 99 100 101 102 103 104 105 106)
fi

dispatch_id="$(date -u +%Y%m%dT%H%M%SZ)_deferred_dispatch"
dispatch_dir="$BASE/state/deferred_dispatch/$dispatch_id"
mkdir -p "$dispatch_dir"

{
  echo "started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "mode=$mode"
  echo "recommended_concurrency=$recommended"
  echo "concurrency=$concurrency"
  echo "repo_commit=$commit"
  echo "versions=${versions[*]}"
} > "$dispatch_dir/meta.env"

active=0
lanes=()

is_lane_running() {
  local v="$1"
  pgrep -f "run_deferred_lane.sh ${v} " >/dev/null 2>&1
}

start_lane() {
  local v="$1"
  local lane="v${v}-kani"
  local lane_dir="$BASE/state/deferred_lanes/$lane"
  mkdir -p "$lane_dir"

  if [[ -f "$lane_dir/exit_code" ]]; then
    echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) skip lane=$lane reason=already-finished code=$(cat "$lane_dir/exit_code")" | tee -a "$dispatch_dir/dispatch.log"
    return
  fi
  if is_lane_running "$v"; then
    echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) skip lane=$lane reason=already-running" | tee -a "$dispatch_dir/dispatch.log"
    return
  fi

  (
    set +e
    echo "start_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$lane_dir/meta.env"
    echo "version=$v" >> "$lane_dir/meta.env"
    echo "mode=$mode" >> "$lane_dir/meta.env"
    "$BASE/bin/run_deferred_lane.sh" "$v" "$mode" > "$lane_dir/run.log" 2>&1
    code=$?
    echo "$code" > "$lane_dir/exit_code"
    echo "end_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$lane_dir/meta.env"
    exit "$code"
  ) &
  local pid=$!
  lanes+=("$lane")
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) started lane=$lane pid=$pid" | tee -a "$dispatch_dir/dispatch.log"
  active=$((active + 1))
}

for v in "${versions[@]}"; do
  while (( active >= concurrency )); do
    if wait -n; then :; fi
    active=$((active - 1))
  done
  start_lane "$v"
done

while (( active > 0 )); do
  if wait -n; then :; fi
  active=$((active - 1))
done

failures=0
for lane in "${lanes[@]}"; do
  code_file="$BASE/state/deferred_lanes/$lane/exit_code"
  code="$(cat "$code_file" 2>/dev/null || echo 99)"
  echo "lane=$lane exit_code=$code" | tee -a "$dispatch_dir/dispatch.log"
  if [[ "$code" != "0" ]]; then
    failures=$((failures + 1))
  fi
done

echo "finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$dispatch_dir/meta.env"
echo "failures=$failures" >> "$dispatch_dir/meta.env"

if (( failures > 0 )); then
  exit 1
fi
EOF

cat > "$BASE/bin/deferred_status.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source "__BASE__/bin/env.sh"
for d in "$BASE"/state/deferred_lanes/*; do
  [[ -d "$d" ]] || continue
  lane="$(basename "$d")"
  version="$(echo "$lane" | sed -E 's/^v([0-9]+)-.*/\1/')"
  code="$(cat "$d/exit_code" 2>/dev/null || true)"
  if [[ -n "$code" ]]; then
    echo "$lane finished:$code"
  else
    pid="$(pgrep -f "run_deferred_lane.sh ${version} " | head -n 1 || true)"
    if [[ -n "$pid" ]]; then
      echo "$lane running:pid=$pid"
    else
      echo "$lane pending"
    fi
  fi
done | sort
EOF

chmod +x "$BASE"/bin/*.sh
chmod +x "$BASE/bin/run_formal_lane.py"

echo "remote-kani: ensured at $BASE"
ls -la "$BASE/bin"
'@

    return ($template -replace '__BASE__', $BaseDir)
}

function Escape-BashSingleQuoted([string]$Value) {
    return $Value.Replace("'", "")
}

Require-Command ssh
Require-Command scp

switch ($Action) {
    "Ensure" {
        $out = Invoke-RemoteScript (Get-EnsureScript -BaseDir $RemoteBase)
        $out | ForEach-Object { Write-Host $_ }
    }
    "ProbeCapacity" {
        $probeTemplate = @'
set -euo pipefail
source "__BASE__/bin/env.sh"
"__BASE__/bin/probe_capacity.sh"
'@
        $probeScript = $probeTemplate.Replace("__BASE__", $RemoteBase)
        $out = Invoke-RemoteScript $probeScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "StartDeferred" {
        $startTemplate = @'
set -euo pipefail
source "__BASE__/bin/env.sh"
if [[ ! -x "__BASE__/tools/bin/cargo-kani" ]]; then
  "__BASE__/bin/start_job.sh" bootstrap "__BASE__/bin/bootstrap_kani.sh"
  echo "cargo-kani not installed; bootstrap job started"
  exit 0
fi
versions='__VERSIONS__'
mode='__MODE__'
concurrency='__CONCURRENCY__'
job_name='__JOB__'
if [[ -n "$versions" ]]; then
  if [[ "$concurrency" -gt 0 ]]; then
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_VERSIONS=$versions" "DEFERRED_CONCURRENCY=$concurrency" "__BASE__/bin/dispatch_deferred_lanes.sh"
  else
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_VERSIONS=$versions" "__BASE__/bin/dispatch_deferred_lanes.sh"
  fi
else
  if [[ "$concurrency" -gt 0 ]]; then
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_CONCURRENCY=$concurrency" "__BASE__/bin/dispatch_deferred_lanes.sh"
  else
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "__BASE__/bin/dispatch_deferred_lanes.sh"
  fi
fi
'@
        $startScript = $startTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__VERSIONS__", (Escape-BashSingleQuoted $DeferredVersions)).
            Replace("__MODE__", (Escape-BashSingleQuoted $DeferredMode)).
            Replace("__CONCURRENCY__", (Escape-BashSingleQuoted $DeferredConcurrency.ToString())).
            Replace("__JOB__", (Escape-BashSingleQuoted $DispatchJobName))
        $out = Invoke-RemoteScript $startScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "Status" {
        $statusTemplate = @'
set -euo pipefail
source "__BASE__/bin/env.sh"
"__BASE__/bin/list_jobs.sh"
echo "---"
"__BASE__/bin/deferred_status.sh" || true
echo "---"
latest_dispatch="$(ls -1dt __BASE__/state/deferred_dispatch/* 2>/dev/null | head -n 1 || true)"
if [[ -n "$latest_dispatch" ]]; then
  echo "dispatch_dir=$latest_dispatch"
  cat "$latest_dispatch/meta.env" || true
fi
'@
        $statusScript = $statusTemplate.Replace("__BASE__", $RemoteBase)
        $out = Invoke-RemoteScript $statusScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "Tail" {
        $laneClauseTemplate = if ([string]::IsNullOrWhiteSpace($Lane)) {
            ""
        }
        else {
@'
echo "---"
if [[ -f "__BASE__/state/deferred_lanes/__LANE__/run.log" ]]; then
  echo "lane=__LANE__"
  tail -n __TAILLINES__ "__BASE__/state/deferred_lanes/__LANE__/run.log"
fi
'@
        }
        $laneClause = $laneClauseTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__LANE__", $Lane.Replace("'", "")).
            Replace("__TAILLINES__", $TailLines.ToString())

        $tailTemplate = @'
set -euo pipefail
source "__BASE__/bin/env.sh"
latest_dispatch="$(ls -1dt __BASE__/state/deferred_dispatch/* 2>/dev/null | head -n 1 || true)"
if [[ -n "$latest_dispatch" ]]; then
  echo "dispatch_dir=$latest_dispatch"
  if [[ -f "$latest_dispatch/dispatch.log" ]]; then
    tail -n __TAILLINES__ "$latest_dispatch/dispatch.log"
  fi
fi
$__LANECLAUSE__
'@
        $tailScript = $tailTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__TAILLINES__", $TailLines.ToString()).
            Replace('$__LANECLAUSE__', $laneClause)
        $out = Invoke-RemoteScript $tailScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "FetchArtifacts" {
        $bundleTemplate = @'
set -euo pipefail
source "__BASE__/bin/env.sh"
ts="$(date -u +%Y%m%dT%H%M%SZ)"
out="__BASE__/artifacts/deferred_export_${ts}.tar.gz"
if [[ -d "__BASE__/state" ]]; then
  tar -czf "$out" -C "__BASE__/state" jobs deferred_dispatch deferred_lanes 2>/dev/null || tar -czf "$out" -C "__BASE__/state" jobs deferred_dispatch 2>/dev/null || tar -czf "$out" -C "__BASE__/state" jobs
  echo "$out"
else
  echo "missing-state-dir" >&2
  exit 1
fi
'@
        $bundleScript = $bundleTemplate.Replace("__BASE__", $RemoteBase)
        $out = Invoke-RemoteScript $bundleScript
        $remoteBundle = ($out | Select-Object -Last 1).Trim()
        if ([string]::IsNullOrWhiteSpace($remoteBundle)) {
            throw "Did not receive artifact bundle path from remote host"
        }

        New-Item -ItemType Directory -Force -Path $LocalArtifactsDir | Out-Null
        $localBundle = Join-Path $LocalArtifactsDir ([System.IO.Path]::GetFileName($remoteBundle))
        & scp -i $SshKeyPath "$SshUser@$SshHost`:$remoteBundle" $localBundle
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to fetch remote bundle: $remoteBundle"
        }
        Write-Host "fetched: $localBundle"
    }
}
