param(
    [ValidateSet("Ensure", "ProbeCapacity", "StartDeferred", "StopDeferred", "Status", "Tail", "FetchArtifacts", "Monitor")]
    [string]$Action = "Status",
    [string]$SshHost = "94.72.99.81",
    [string]$SshUser = "ubuntu",
    [string]$SshKeyPath = "$env:USERPROFILE\.ssh\acfs_ed25519",
    [string]$RemoteBase = "/home/ubuntu/.dnacalc_remote",
    [ValidateSet("cumulative", "exact")]
    [string]$DeferredMode = "cumulative",
    [ValidateSet("lane", "dedup")]
    [string]$DeferredStrategy = "dedup",
    [string]$DeferredVersions = "",
    [int]$DeferredConcurrency = 0,
    [int]$ObligationTimeoutSeconds = 10800,
    [int]$ObligationTimeoutRetries = 1,
    [double]$ObligationTimeoutMultiplier = 10.0,
    [int]$MemorySoftUsedPercent = 85,
    [int]$MemoryHardUsedPercent = 92,
    [ValidateSet("pause", "halt-one", "halt-all", "none")]
    [string]$HardPressureAction = "pause",
    [ValidateSet("stale", "all")]
    [string]$StopMode = "stale",
    [string]$DispatchJobName = "deferred-dispatch",
    [int]$TailLines = 80,
    [string]$Lane = "",
    [string]$LocalArtifactsDir = "temp/async/kani_remote",
    [int]$MonitorDurationSeconds = 0,
    [int]$MonitorIntervalSeconds = 30,
    [bool]$MonitorAutoResume = $true
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$SshCommonOptions = @(
    "-o", "ServerAliveInterval=15",
    "-o", "ServerAliveCountMax=4",
    "-o", "TCPKeepAlive=yes",
    "-o", "ConnectTimeout=10"
)

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
    $out = $ScriptText | & ssh @SshCommonOptions -i $SshKeyPath $sshTarget $remoteCommand
    if ($LASTEXITCODE -ne 0) {
        throw "Remote command failed with exit code $LASTEXITCODE"
    }
    return $out
}

function Get-EnsureScript([string]$BaseDir) {
    $template = @'
set -euo pipefail
BASE="__BASE__"
mkdir -p "$BASE"/{bin,logs,state/jobs,state/deferred_lanes,state/deferred_dispatch,state/dedup,artifacts,tmp,home,cargo,rustup,work,tools}

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
mkdir -p "$TMPDIR" "$HOME" "$CARGO_HOME" "$RUSTUP_HOME" "$BASE/logs" "$BASE/state/jobs" "$BASE/state/dedup" "$BASE/artifacts" "$BASE/work"
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
    cmd="$(cat "$d/command.txt" 2>/dev/null || true)"
    if [[ "$cmd" == *"dispatch_deferred_lanes.sh"* ]] && pgrep -f "__BASE__/bin/dispatch_deferred_lanes.sh" >/dev/null 2>&1; then
      state="running-detached"
    elif [[ "$cmd" == *"run_deferred_lane.sh"* ]]; then
      v="$(echo "$cmd" | sed -nE 's/.*run_deferred_lane\.sh ([0-9]+).*/\1/p' | head -n 1)"
      if [[ -n "$v" ]] && pgrep -f "__BASE__/bin/run_deferred_lane.sh ${v} " >/dev/null 2>&1; then
        state="running-detached"
      else
        state="unknown"
      fi
    else
      state="unknown"
    fi
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
mem_available_kib="$(awk '/MemAvailable/ {print $2}' /proc/meminfo)"
mem_total_kib="$(awk '/MemTotal/ {print $2}' /proc/meminfo)"
mem_gib=$((mem_available_kib / 1024 / 1024))
mem_total_gib=$((mem_total_kib / 1024 / 1024))
mem_used_pct=0
if (( mem_total_kib > 0 )); then
  mem_used_pct=$(( (100 * (mem_total_kib - mem_available_kib)) / mem_total_kib ))
fi
by_cpu=$((cpu / 8))
by_mem=$((mem_gib / 24))
if (( by_cpu < 1 )); then by_cpu=1; fi
if (( by_mem < 1 )); then by_mem=1; fi
conc="$by_cpu"
if (( by_mem < conc )); then conc="$by_mem"; fi
if (( conc > 4 )); then conc=4; fi
load1="$(awk '{print $1}' /proc/loadavg)"
printf 'cpu=%s\nmem_total_gib=%s\nmem_available_gib=%s\nmem_used_percent=%s\nload1=%s\nrecommended_concurrency=%s\n' "$cpu" "$mem_total_gib" "$mem_gib" "$mem_used_pct" "$load1" "$conc"
EOF

cat > "$BASE/bin/resource_snapshot.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cpu="$(nproc)"
mem_total_kib="$(awk '/MemTotal/ {print $2}' /proc/meminfo)"
mem_available_kib="$(awk '/MemAvailable/ {print $2}' /proc/meminfo)"
swap_total_kib="$(awk '/SwapTotal/ {print $2}' /proc/meminfo)"
swap_free_kib="$(awk '/SwapFree/ {print $2}' /proc/meminfo)"
mem_used_pct=0
if (( mem_total_kib > 0 )); then
  mem_used_pct=$(( (100 * (mem_total_kib - mem_available_kib)) / mem_total_kib ))
fi
swap_used_kib=$((swap_total_kib - swap_free_kib))
load_line="$(cat /proc/loadavg)"
load1="$(awk '{print $1}' /proc/loadavg)"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
cbmc_count="$(pgrep -fc 'cbmc .*' || true)"
kani_count="$(pgrep -fc 'cargo-kani|cargo kani' || true)"
printf 'timestamp_utc=%s\ncpu=%s\nmem_total_kib=%s\nmem_available_kib=%s\nmem_used_percent=%s\nswap_total_kib=%s\nswap_used_kib=%s\nload1=%s\nloadavg=%s\ncbmc_count=%s\nkani_count=%s\n' \
  "$ts" "$cpu" "$mem_total_kib" "$mem_available_kib" "$mem_used_pct" "$swap_total_kib" "$swap_used_kib" "$load1" "$load_line" "$cbmc_count" "$kani_count"

if pgrep -f "__BASE__/tools/bin/cargo-kani|cbmc .*__BASE__/work/targets/" >/dev/null 2>&1; then
  echo "top_processes_begin"
  ps -eo pid,rss,comm,args --sort=-rss | awk '
    /cargo-kani|cbmc/ && $0 !~ /awk/ {
      rss_mib = $2 / 1024.0;
      printf("pid=%s rss_mib=%.1f comm=%s cmd=%s\n", $1, rss_mib, $3, substr($0, index($0,$4)));
      shown++;
      if (shown >= 8) exit 0;
    }
  '
  echo "top_processes_end"
fi
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
import hashlib
import csv
import datetime as dt
import json
import os
import pathlib
import re
import subprocess
import sys
import time
import shutil
import selectors
from contextlib import nullcontext
from typing import Optional

def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

def parse_version(profile: str) -> int:
    m = re.search(r"v(\d+)$", (profile or "").strip())
    if not m:
        return -1
    return int(m.group(1))

def atomic_write_text(path: pathlib.Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)

def atomic_write_json(path: pathlib.Path, payload: dict) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2) + "\n")

def shell_capture(cmd: str, cwd: pathlib.Path | None = None, timeout: int = 30) -> str:
    proc = subprocess.run(
        ["bash", "-lc", cmd],
        cwd=str(cwd) if cwd else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=timeout,
    )
    if proc.returncode != 0:
        return ""
    return (proc.stdout or "").strip()

def command_key(commit: str, tool: str, cmd: str, timeout_seconds: int) -> str:
    h = hashlib.sha256()
    h.update(commit.encode("utf-8", "replace"))
    h.update(b"\n")
    h.update(tool.encode("utf-8", "replace"))
    h.update(b"\n")
    h.update(cmd.encode("utf-8", "replace"))
    h.update(b"\n")
    h.update(str(timeout_seconds).encode("utf-8", "replace"))
    return h.hexdigest()

def acquire_cache_lock(lock_path: pathlib.Path, cache_json: pathlib.Path, progress_cb, stale_seconds: int = 86400) -> bool:
    while True:
        if cache_json.exists():
            return False
        try:
            fd = os.open(str(lock_path), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(now_utc())
            return True
        except FileExistsError:
            try:
                age = time.time() - lock_path.stat().st_mtime
                if age > stale_seconds:
                    lock_path.unlink(missing_ok=True)
                    continue
            except FileNotFoundError:
                continue
            progress_cb("waiting-cache-lock")
            time.sleep(2)

def stream_command(
    cmd: str,
    cwd: pathlib.Path,
    obligation_log_path: pathlib.Path,
    lane_log_path: Optional[pathlib.Path],
    timeout_seconds: int,
    progress_cb,
) -> tuple[int, bool, int, str]:
    obligation_log_path.parent.mkdir(parents=True, exist_ok=True)
    lane_ctx = lane_log_path.open("ab") if lane_log_path else nullcontext()
    started_ts = time.monotonic()
    last_output = now_utc()
    output_bytes = 0
    timed_out = False

    with obligation_log_path.open("wb") as obl_log, lane_ctx as lane_log:
        proc = subprocess.Popen(
            ["bash", "-lc", cmd],
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=False,
            bufsize=0,
        )
        assert proc.stdout is not None
        fd = proc.stdout.fileno()
        sel = selectors.DefaultSelector()
        sel.register(fd, selectors.EVENT_READ)

        while True:
            elapsed = int(time.monotonic() - started_ts)
            if timeout_seconds > 0 and elapsed > timeout_seconds:
                timed_out = True
                try:
                    proc.kill()
                except ProcessLookupError:
                    pass
                break

            events = sel.select(timeout=1.0)
            if events:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    data = b""
                if data:
                    output_bytes += len(data)
                    obl_log.write(data)
                    obl_log.flush()
                    if lane_log is not None:
                        lane_log.write(data)
                        lane_log.flush()
                    last_output = now_utc()
                elif proc.poll() is not None:
                    break

            progress_cb("running-command", elapsed_seconds=elapsed, last_output_utc=last_output, output_bytes=output_bytes)

            if proc.poll() is not None and not events:
                break

        if timed_out:
            tail = f"\n[runner] timeout after {timeout_seconds}s; process killed\n".encode("utf-8")
            obl_log.write(tail)
            obl_log.flush()
            if lane_log is not None:
                lane_log.write(tail)
                lane_log.flush()

        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            try:
                proc.kill()
            except ProcessLookupError:
                pass
            proc.wait(timeout=10)

        exit_code = proc.returncode if proc.returncode is not None else 99
        if timed_out:
            exit_code = 124
        return exit_code, timed_out, output_bytes, last_output

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--target-version", type=int, required=True)
    ap.add_argument("--mode", choices=["cumulative", "exact"], default="cumulative")
    ap.add_argument("--dedup-strategy", choices=["lane", "dedup"], default="dedup")
    ap.add_argument("--filter", choices=["kani", "all"], default="kani")
    ap.add_argument("--obligations", default="docs/evidence/formal/obligations.csv")
    ap.add_argument("--report-dir", required=True)
    ap.add_argument("--cache-dir", default="")
    ap.add_argument("--lane-log", default="")
    ap.add_argument("--heartbeat-file", default="")
    ap.add_argument("--timeout-seconds", type=int, default=10800)
    ap.add_argument("--timeout-retries", type=int, default=1)
    ap.add_argument("--timeout-multiplier", type=float, default=10.0)
    args = ap.parse_args()
    if args.timeout_seconds < 1:
        args.timeout_seconds = 1
    if args.timeout_retries < 0:
        args.timeout_retries = 0
    if args.timeout_multiplier < 1.0:
        args.timeout_multiplier = 1.0

    repo = pathlib.Path(args.repo).resolve()
    obligations_path = (repo / args.obligations).resolve()
    report_dir = pathlib.Path(args.report_dir).resolve()
    report_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = pathlib.Path(args.cache_dir).resolve() if args.cache_dir else (report_dir / "_cache")
    cache_dir.mkdir(parents=True, exist_ok=True)
    lane_log_path = pathlib.Path(args.lane_log).resolve() if args.lane_log else None
    heartbeat_file = pathlib.Path(args.heartbeat_file).resolve() if args.heartbeat_file else (report_dir / "progress.json")
    status_file = report_dir / "status.txt"

    if not obligations_path.exists():
        raise SystemExit(f"missing obligations file: {obligations_path}")

    repo_commit = shell_capture("git rev-parse HEAD", cwd=repo, timeout=20)
    if not repo_commit:
        repo_commit = "unknown-commit"
    kani_version = shell_capture("cargo kani --version", cwd=repo, timeout=30)
    if not kani_version:
        kani_version = "unknown-kani-version"

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

    atomic_write_text(status_file, "running\n")
    atomic_write_json(
        heartbeat_file,
        {
            "timestamp_utc": now_utc(),
            "phase": "selected",
            "repo_commit": repo_commit,
            "kani_version": kani_version,
            "target_version": args.target_version,
            "mode": args.mode,
            "dedup_strategy": args.dedup_strategy,
            "selected_count": len(selected),
            "completed_count": 0,
            "failures": 0,
            "timeouts": 0,
            "cache_hits": 0,
            "timeout_seconds": args.timeout_seconds,
            "timeout_retries": args.timeout_retries,
            "timeout_multiplier": args.timeout_multiplier,
        },
    )

    rows = []
    cache_hits = 0
    attempt_history: dict[str, list[dict]] = {}

    def execute_obligation(row: dict, queue_index: int, queue_total: int, attempt: int, retry_round: int, timeout_seconds: int) -> dict:
        nonlocal cache_hits
        obligation = row.get("obligation_id", "unknown")
        cmd = (row.get("command") or "").strip()
        suffix = "" if attempt == 1 else f".attempt{attempt}"
        obligation_log_path = report_dir / f"{obligation}{suffix}.log"
        started = now_utc()
        cache_hit = False

        def update_progress(phase: str, **extra):
            payload = {
                "timestamp_utc": now_utc(),
                "phase": phase,
                "repo_commit": repo_commit,
                "kani_version": kani_version,
                "target_version": args.target_version,
                "mode": args.mode,
                "dedup_strategy": args.dedup_strategy,
                "selected_count": len(selected),
                "completed_count": sum(len(v) for v in attempt_history.values()),
                "current_obligation": obligation,
                "current_profile": row.get("profile", ""),
                "queue_index": queue_index,
                "queue_total": queue_total,
                "attempt": attempt,
                "retry_round": retry_round,
                "timeout_seconds": timeout_seconds,
                "cache_hits": cache_hits,
            }
            payload.update(extra)
            atomic_write_json(heartbeat_file, payload)

        update_progress("preparing")
        key = command_key(repo_commit, kani_version, cmd, timeout_seconds)
        cache_json = cache_dir / f"{key}.json"
        cache_log = cache_dir / f"{key}.log"
        lock_path = cache_dir / f"{key}.lock"

        if args.dedup_strategy == "dedup":
            owner = acquire_cache_lock(lock_path, cache_json, update_progress)
            if owner:
                cache_tmp = cache_dir / f"{key}.tmp.log"
                exit_code, timed_out, output_bytes, last_output = stream_command(
                    cmd=cmd,
                    cwd=repo,
                    obligation_log_path=obligation_log_path,
                    lane_log_path=lane_log_path,
                    timeout_seconds=timeout_seconds,
                    progress_cb=update_progress,
                )
                shutil.copy2(obligation_log_path, cache_tmp)
                cache_tmp.replace(cache_log)
                cache_payload = {
                    "timestamp_utc": now_utc(),
                    "repo_commit": repo_commit,
                    "kani_version": kani_version,
                    "command": cmd,
                    "timeout_seconds": timeout_seconds,
                    "exit_code": exit_code,
                    "timed_out": timed_out,
                    "output_bytes": output_bytes,
                    "last_output_utc": last_output,
                }
                atomic_write_json(cache_json, cache_payload)
                lock_path.unlink(missing_ok=True)
            else:
                while not cache_json.exists():
                    update_progress("waiting-cache-result")
                    time.sleep(1)
                cache_hit = True
                cache_hits += 1
                cached = json.loads(cache_json.read_text(encoding="utf-8"))
                exit_code = int(cached.get("exit_code", 99))
                timed_out = bool(cached.get("timed_out", False))
                if cache_log.exists():
                    shutil.copy2(cache_log, obligation_log_path)
                else:
                    obligation_log_path.write_text("[runner] cache-hit without cache log\n", encoding="utf-8")
                if lane_log_path:
                    with lane_log_path.open("ab") as lane_log:
                        lane_log.write(
                            f"[runner] cache-hit obligation={obligation} timeout_seconds={timeout_seconds} command={cmd}\n".encode("utf-8")
                        )
                        lane_log.flush()
        else:
            exit_code, timed_out, output_bytes, last_output = stream_command(
                cmd=cmd,
                cwd=repo,
                obligation_log_path=obligation_log_path,
                lane_log_path=lane_log_path,
                timeout_seconds=timeout_seconds,
                progress_cb=update_progress,
            )

        ended = now_utc()
        if timed_out:
            status = "timeout"
        elif exit_code == 0:
            status = "pass"
        else:
            status = "fail"

        result = {
            "obligation": obligation,
            "profile": row.get("profile", ""),
            "command": cmd,
            "status": status,
            "cache_hit": "true" if cache_hit else "false",
            "exit_code": exit_code,
            "artifact": row.get("artifact", ""),
            "started_utc": started,
            "ended_utc": ended,
            "log": str(obligation_log_path),
            "attempt": attempt,
            "retry_round": retry_round,
            "timeout_seconds": timeout_seconds,
        }
        atomic_write_json(
            heartbeat_file,
            {
                "timestamp_utc": now_utc(),
                "phase": "obligation-complete",
                "repo_commit": repo_commit,
                "kani_version": kani_version,
                "target_version": args.target_version,
                "mode": args.mode,
                "dedup_strategy": args.dedup_strategy,
                "selected_count": len(selected),
                "completed_count": sum(len(v) for v in attempt_history.values()) + 1,
                "current_obligation": obligation,
                "last_status": status,
                "attempt": attempt,
                "retry_round": retry_round,
                "timeout_seconds": timeout_seconds,
                "cache_hits": cache_hits,
            },
        )
        return result

    timed_out_queue: list[tuple[dict, int]] = []
    for idx, row in enumerate(selected, start=1):
        result = execute_obligation(
            row=row,
            queue_index=idx,
            queue_total=len(selected),
            attempt=1,
            retry_round=0,
            timeout_seconds=args.timeout_seconds,
        )
        history = attempt_history.setdefault(result["obligation"], [])
        history.append(result)
        if result["status"] == "timeout" and args.timeout_retries > 0:
            timed_out_queue.append((row, 2))

    for retry_round in range(1, args.timeout_retries + 1):
        if not timed_out_queue:
            break
        current_queue = timed_out_queue
        timed_out_queue = []
        retry_timeout = max(args.timeout_seconds, int(round(args.timeout_seconds * (args.timeout_multiplier ** retry_round))))
        atomic_write_json(
            heartbeat_file,
            {
                "timestamp_utc": now_utc(),
                "phase": "retry-queue",
                "repo_commit": repo_commit,
                "kani_version": kani_version,
                "target_version": args.target_version,
                "mode": args.mode,
                "dedup_strategy": args.dedup_strategy,
                "selected_count": len(selected),
                "retry_round": retry_round,
                "retry_count": len(current_queue),
                "timeout_seconds": retry_timeout,
                "cache_hits": cache_hits,
            },
        )
        for idx, (row, attempt) in enumerate(current_queue, start=1):
            result = execute_obligation(
                row=row,
                queue_index=idx,
                queue_total=len(current_queue),
                attempt=attempt,
                retry_round=retry_round,
                timeout_seconds=retry_timeout,
            )
            history = attempt_history.setdefault(result["obligation"], [])
            history.append(result)
            if result["status"] == "timeout" and retry_round < args.timeout_retries:
                timed_out_queue.append((row, attempt + 1))

    for row in selected:
        obligation = row.get("obligation_id", "unknown")
        history = attempt_history.get(obligation, [])
        if not history:
            continue
        final = dict(history[-1])
        final["attempts"] = len(history)
        final["initial_status"] = history[0]["status"]
        rows.append(final)

    selected_total = len(rows)
    failures = sum(1 for row in rows if row["status"] != "pass")
    timeouts = sum(1 for row in rows if row["status"] == "timeout")
    if selected_total == 0:
        lane_status = "no-op"
    else:
        lane_status = "pass" if failures == 0 else "fail"

    csv_path = report_dir / "formal_lane.csv"
    with csv_path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "obligation",
                "profile",
                "status",
                "initial_status",
                "attempts",
                "attempt",
                "retry_round",
                "timeout_seconds",
                "cache_hit",
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
        "repo_commit": repo_commit,
        "kani_version": kani_version,
        "target_version": args.target_version,
        "mode": args.mode,
        "dedup_strategy": args.dedup_strategy,
        "filter": args.filter,
        "selected_count": selected_total,
        "failures": failures,
        "timeouts": timeouts,
        "cache_hits": cache_hits,
        "timeout_seconds": args.timeout_seconds,
        "timeout_retries": args.timeout_retries,
        "timeout_multiplier": args.timeout_multiplier,
        "status": lane_status,
        "report_csv": str(csv_path),
    }
    (report_dir / "formal_lane.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    md_lines = [
        "# Remote Formal Lane",
        "",
        f"- Timestamp (UTC): {summary['timestamp_utc']}",
        f"- Target version: v{args.target_version}",
        f"- Mode: {args.mode}",
        f"- Strategy: {args.dedup_strategy}",
        f"- Filter: {args.filter}",
        f"- Selected obligations: {selected_total}",
        f"- Failures: {failures}",
        f"- Timeouts: {timeouts}",
        f"- Cache hits: {cache_hits}",
        f"- Timeout retries: {args.timeout_retries}",
        f"- Timeout multiplier: {args.timeout_multiplier}",
        f"- Status: {summary['status']}",
        "",
        "| Obligation | Profile | Status | Initial | Attempts | Timeout(s) | Cache | Exit | Command |",
        "|---|---|---|---|---:|---:|---|---:|---|",
    ]
    for row in rows:
        md_lines.append(
            f"| {row['obligation']} | {row['profile']} | {row['status']} | {row['initial_status']} | {row['attempts']} | {row['timeout_seconds']} | {row['cache_hit']} | {row['exit_code']} | {row['command']} |"
        )
    (report_dir / "formal_lane.md").write_text("\n".join(md_lines) + "\n", encoding="utf-8")

    atomic_write_text(
        report_dir / "summary.txt",
        (
            f"status={summary['status']}\n"
            f"selected_count={selected_total}\n"
            f"failures={failures}\n"
            f"timeouts={timeouts}\n"
            f"cache_hits={cache_hits}\n"
            f"timeout_retries={args.timeout_retries}\n"
            f"timeout_multiplier={args.timeout_multiplier}\n"
            f"timestamp_utc={summary['timestamp_utc']}\n"
        ),
    )
    atomic_write_text(status_file, f"completed:{summary['status']}\n")
    atomic_write_json(
        heartbeat_file,
        {
            "timestamp_utc": now_utc(),
            "phase": "completed",
            "repo_commit": repo_commit,
            "kani_version": kani_version,
            "target_version": args.target_version,
            "mode": args.mode,
            "dedup_strategy": args.dedup_strategy,
            "selected_count": selected_total,
            "completed_count": selected_total,
            "failures": failures,
            "timeouts": timeouts,
            "cache_hits": cache_hits,
            "timeout_seconds": args.timeout_seconds,
            "timeout_retries": args.timeout_retries,
            "timeout_multiplier": args.timeout_multiplier,
            "status": summary["status"],
        },
    )

    if lane_status == "no-op":
        return 2
    return 0 if lane_status == "pass" else 1

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
strategy="${DEFERRED_STRATEGY:-dedup}"
timeout_seconds="${OBLIGATION_TIMEOUT_SECONDS:-10800}"
timeout_retries="${OBLIGATION_TIMEOUT_RETRIES:-1}"
timeout_multiplier="${OBLIGATION_TIMEOUT_MULTIPLIER:-10}"
mem_soft_used_percent="${MEM_SOFT_USED_PERCENT:-85}"
mem_hard_used_percent="${MEM_HARD_USED_PERCENT:-92}"
hard_pressure_action="${HARD_PRESSURE_ACTION:-pause}"
pause_flag_file="$BASE/state/deferred_dispatch/PAUSE_NEW_LANES.auto"
manual_pause_file="$BASE/state/deferred_dispatch/PAUSE_NEW_LANES.manual"
python3 "$BASE/bin/run_formal_lane.py" \
  --repo "$repo" \
  --target-version "$v" \
  --mode "$mode" \
  --dedup-strategy "$strategy" \
  --filter kani \
  --report-dir "$lane_dir" \
  --cache-dir "$BASE/state/dedup" \
  --lane-log "$lane_dir/run.log" \
  --heartbeat-file "$lane_dir/progress.json" \
  --timeout-seconds "$timeout_seconds" \
  --timeout-retries "$timeout_retries" \
  --timeout-multiplier "$timeout_multiplier"
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
strategy="${DEFERRED_STRATEGY:-dedup}"
timeout_seconds="${OBLIGATION_TIMEOUT_SECONDS:-10800}"
timeout_retries="${OBLIGATION_TIMEOUT_RETRIES:-1}"
timeout_multiplier="${OBLIGATION_TIMEOUT_MULTIPLIER:-10}"

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
  echo "strategy=$strategy"
  echo "obligation_timeout_seconds=$timeout_seconds"
  echo "obligation_timeout_retries=$timeout_retries"
  echo "obligation_timeout_multiplier=$timeout_multiplier"
  echo "mem_soft_used_percent=$mem_soft_used_percent"
  echo "mem_hard_used_percent=$mem_hard_used_percent"
  echo "hard_pressure_action=$hard_pressure_action"
  echo "repo_commit=$commit"
  echo "versions=${versions[*]}"
} > "$dispatch_dir/meta.env"
echo "running" > "$dispatch_dir/state.txt"

active=0
lanes=()

is_lane_running() {
  local v="$1"
  pgrep -f "run_deferred_lane.sh ${v} " >/dev/null 2>&1
}

mem_used_percent() {
  local total available
  total="$(awk '/MemTotal/ {print $2}' /proc/meminfo)"
  available="$(awk '/MemAvailable/ {print $2}' /proc/meminfo)"
  if [[ -z "$total" || "$total" == "0" || -z "$available" ]]; then
    echo 0
    return
  fi
  echo $(( (100 * (total - available)) / total ))
}

has_pause_flag() {
  [[ -f "$pause_flag_file" || -f "$manual_pause_file" ]]
}

enforce_pressure_policy() {
  local used
  used="$(mem_used_percent)"
  if (( used < mem_soft_used_percent )); then
    if [[ -f "$pause_flag_file" ]]; then
      rm -f "$pause_flag_file"
      echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard action=resume reason=memory-recovered mem_used_percent=$used" | tee -a "$dispatch_dir/dispatch.log"
    fi
    return 0
  fi

  if (( used >= mem_hard_used_percent )); then
    case "$hard_pressure_action" in
      halt-one)
        pid="$(ps -eo pid,rss,args --sort=-rss | awk '/cargo-kani|cbmc/ && $0 !~ /awk/ {print $1; exit}')"
        if [[ -n "${pid:-}" ]]; then
          kill -TERM "$pid" 2>/dev/null || true
          sleep 2
          if kill -0 "$pid" 2>/dev/null; then
            kill -KILL "$pid" 2>/dev/null || true
          fi
          echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard action=halt-one mem_used_percent=$used pid=$pid" | tee -a "$dispatch_dir/dispatch.log"
        fi
        ;;
      halt-all)
        mapfile -t pids < <(pgrep -f "__BASE__/tools/bin/cargo-kani|cbmc .*__BASE__/work/targets/" || true)
        for p in "${pids[@]:-}"; do
          kill -TERM "$p" 2>/dev/null || true
        done
        sleep 2
        for p in "${pids[@]:-}"; do
          if kill -0 "$p" 2>/dev/null; then
            kill -KILL "$p" 2>/dev/null || true
          fi
        done
        echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard action=halt-all mem_used_percent=$used count=${#pids[@]}" | tee -a "$dispatch_dir/dispatch.log"
        ;;
      pause|*)
        touch "$pause_flag_file"
        echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard action=pause mem_used_percent=$used" | tee -a "$dispatch_dir/dispatch.log"
        ;;
    esac
  fi

  if (( used >= mem_soft_used_percent )); then
    return 1
  fi
  return 0
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
    echo "strategy=$strategy" >> "$lane_dir/meta.env"
    echo "timeout_seconds=$timeout_seconds" >> "$lane_dir/meta.env"
    echo "timeout_retries=$timeout_retries" >> "$lane_dir/meta.env"
    echo "timeout_multiplier=$timeout_multiplier" >> "$lane_dir/meta.env"
    echo "mem_soft_used_percent=$mem_soft_used_percent" >> "$lane_dir/meta.env"
    echo "mem_hard_used_percent=$mem_hard_used_percent" >> "$lane_dir/meta.env"
    echo "hard_pressure_action=$hard_pressure_action" >> "$lane_dir/meta.env"
    DEFERRED_STRATEGY="$strategy" OBLIGATION_TIMEOUT_SECONDS="$timeout_seconds" OBLIGATION_TIMEOUT_RETRIES="$timeout_retries" OBLIGATION_TIMEOUT_MULTIPLIER="$timeout_multiplier" \
      "$BASE/bin/run_deferred_lane.sh" "$v" "$mode" > "$lane_dir/driver.log" 2>&1
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
  while true; do
    if has_pause_flag; then
      echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard wait reason=pause-flag lane=v${v}-kani" | tee -a "$dispatch_dir/dispatch.log"
      sleep 15
      continue
    fi
    if enforce_pressure_policy; then
      break
    fi
    used_now="$(mem_used_percent)"
    echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) guard wait reason=memory-pressure lane=v${v}-kani mem_used_percent=$used_now soft=$mem_soft_used_percent hard=$mem_hard_used_percent action=$hard_pressure_action" | tee -a "$dispatch_dir/dispatch.log"
    sleep 15
  done
  start_lane "$v"
done

while (( active > 0 )); do
  enforce_pressure_policy || true
  if wait -n; then :; fi
  active=$((active - 1))
done

failures=0
for lane in "${lanes[@]}"; do
  lane_dir="$BASE/state/deferred_lanes/$lane"
  code_file="$lane_dir/exit_code"
  status_file="$lane_dir/status.txt"
  summary_file="$lane_dir/summary.txt"
  code="$(cat "$code_file" 2>/dev/null || echo 99)"
  status_marker="$(cat "$status_file" 2>/dev/null || echo "-")"
  selected_count="$(sed -nE 's/^selected_count=([0-9]+).*/\1/p' "$summary_file" 2>/dev/null | head -n 1 || true)"
  if [[ -z "$selected_count" ]]; then
    selected_count="-"
  fi
  echo "lane=$lane exit_code=$code status=$status_marker selected_count=$selected_count" | tee -a "$dispatch_dir/dispatch.log"
  if [[ "$status_marker" == "completed:no-op" || "$selected_count" == "0" ]]; then
    echo "warning lane=$lane no-op-selected-count-zero probable_commit_obligation_mismatch" | tee -a "$dispatch_dir/dispatch.log"
  fi
  if [[ "$code" != "0" ]]; then
    failures=$((failures + 1))
  fi
done

echo "finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$dispatch_dir/meta.env"
echo "failures=$failures" >> "$dispatch_dir/meta.env"
if (( failures > 0 )); then
  echo "completed:fail" > "$dispatch_dir/state.txt"
else
  echo "completed:pass" > "$dispatch_dir/state.txt"
fi

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
  progress="$d/progress.json"
  status_file="$d/status.txt"
  run_log="$d/run.log"
  log_bytes="$(wc -c "$run_log" 2>/dev/null | awk '{print $1}' || echo 0)"
  if [[ -n "$code" ]]; then
    status_marker="$(cat "$status_file" 2>/dev/null || echo "-")"
    selected_count="$(sed -nE 's/^selected_count=([0-9]+).*/\1/p' "$d/summary.txt" 2>/dev/null | head -n 1 || true)"
    if [[ -z "$selected_count" ]]; then selected_count="-"; fi
    if [[ "$status_marker" == "completed:no-op" || "$selected_count" == "0" ]]; then
      echo "$lane no-op:$code selected_count=$selected_count log_bytes=$log_bytes warning=probable-commit-obligation-mismatch"
    else
      echo "$lane finished:$code selected_count=$selected_count log_bytes=$log_bytes"
    fi
  else
    pid=""
    if [[ "$lane" =~ ^v[0-9]+-kani$ ]]; then
      pid="$(pgrep -f "run_deferred_lane.sh ${version} " | head -n 1 || true)"
    fi
    phase="-"
    completed="-"
    selected="-"
    current="-"
    if [[ -f "$progress" ]]; then
      readarray -t vals < <(python3 - "$progress" <<'PY'
import json, sys
p = json.load(open(sys.argv[1], "r", encoding="utf-8"))
print(p.get("phase", "-"))
print(p.get("completed_count", "-"))
print(p.get("selected_count", "-"))
print(p.get("current_obligation", "-"))
PY
      )
      phase="${vals[0]:--}"
      completed="${vals[1]:--}"
      selected="${vals[2]:--}"
      current="${vals[3]:--}"
    fi
    status_marker="$(cat "$status_file" 2>/dev/null || echo "-")"
    if [[ "$status_marker" == completed:* && -z "$pid" ]]; then
      echo "$lane finished:unknown phase=$phase progress=${completed}/${selected} current=$current status=$status_marker log_bytes=$log_bytes"
      continue
    fi
    if [[ -n "$pid" ]]; then
      echo "$lane running:pid=$pid phase=$phase progress=${completed}/${selected} current=$current status=$status_marker log_bytes=$log_bytes"
    else
      echo "$lane pending phase=$phase progress=${completed}/${selected} current=$current status=$status_marker log_bytes=$log_bytes"
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

if ($MemorySoftUsedPercent -lt 1 -or $MemorySoftUsedPercent -gt 99) {
    throw "-MemorySoftUsedPercent must be between 1 and 99"
}
if ($MemoryHardUsedPercent -lt 1 -or $MemoryHardUsedPercent -gt 99) {
    throw "-MemoryHardUsedPercent must be between 1 and 99"
}
if ($MemoryHardUsedPercent -lt $MemorySoftUsedPercent) {
    throw "-MemoryHardUsedPercent must be >= -MemorySoftUsedPercent"
}
if ($MonitorIntervalSeconds -lt 1) {
    throw "-MonitorIntervalSeconds must be >= 1"
}

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
strategy='__STRATEGY__'
timeout_seconds='__TIMEOUT__'
timeout_retries='__TIMEOUT_RETRIES__'
timeout_multiplier='__TIMEOUT_MULTIPLIER__'
mem_soft_used_percent='__MEM_SOFT__'
mem_hard_used_percent='__MEM_HARD__'
hard_pressure_action='__HARD_ACTION__'
job_name='__JOB__'
if [[ -n "$versions" ]]; then
  if [[ "$concurrency" -gt 0 ]]; then
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_STRATEGY=$strategy" "OBLIGATION_TIMEOUT_SECONDS=$timeout_seconds" "OBLIGATION_TIMEOUT_RETRIES=$timeout_retries" "OBLIGATION_TIMEOUT_MULTIPLIER=$timeout_multiplier" "MEM_SOFT_USED_PERCENT=$mem_soft_used_percent" "MEM_HARD_USED_PERCENT=$mem_hard_used_percent" "HARD_PRESSURE_ACTION=$hard_pressure_action" "DEFERRED_VERSIONS=$versions" "DEFERRED_CONCURRENCY=$concurrency" "__BASE__/bin/dispatch_deferred_lanes.sh"
  else
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_STRATEGY=$strategy" "OBLIGATION_TIMEOUT_SECONDS=$timeout_seconds" "OBLIGATION_TIMEOUT_RETRIES=$timeout_retries" "OBLIGATION_TIMEOUT_MULTIPLIER=$timeout_multiplier" "MEM_SOFT_USED_PERCENT=$mem_soft_used_percent" "MEM_HARD_USED_PERCENT=$mem_hard_used_percent" "HARD_PRESSURE_ACTION=$hard_pressure_action" "DEFERRED_VERSIONS=$versions" "__BASE__/bin/dispatch_deferred_lanes.sh"
  fi
else
  if [[ "$concurrency" -gt 0 ]]; then
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_STRATEGY=$strategy" "OBLIGATION_TIMEOUT_SECONDS=$timeout_seconds" "OBLIGATION_TIMEOUT_RETRIES=$timeout_retries" "OBLIGATION_TIMEOUT_MULTIPLIER=$timeout_multiplier" "MEM_SOFT_USED_PERCENT=$mem_soft_used_percent" "MEM_HARD_USED_PERCENT=$mem_hard_used_percent" "HARD_PRESSURE_ACTION=$hard_pressure_action" "DEFERRED_CONCURRENCY=$concurrency" "__BASE__/bin/dispatch_deferred_lanes.sh"
  else
    "__BASE__/bin/start_job.sh" "$job_name" env "DEFERRED_MODE=$mode" "DEFERRED_STRATEGY=$strategy" "OBLIGATION_TIMEOUT_SECONDS=$timeout_seconds" "OBLIGATION_TIMEOUT_RETRIES=$timeout_retries" "OBLIGATION_TIMEOUT_MULTIPLIER=$timeout_multiplier" "MEM_SOFT_USED_PERCENT=$mem_soft_used_percent" "MEM_HARD_USED_PERCENT=$mem_hard_used_percent" "HARD_PRESSURE_ACTION=$hard_pressure_action" "__BASE__/bin/dispatch_deferred_lanes.sh"
  fi
fi
'@
        $startScript = $startTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__VERSIONS__", (Escape-BashSingleQuoted $DeferredVersions)).
            Replace("__MODE__", (Escape-BashSingleQuoted $DeferredMode)).
            Replace("__CONCURRENCY__", (Escape-BashSingleQuoted $DeferredConcurrency.ToString())).
            Replace("__STRATEGY__", (Escape-BashSingleQuoted $DeferredStrategy)).
            Replace("__TIMEOUT__", (Escape-BashSingleQuoted $ObligationTimeoutSeconds.ToString())).
            Replace("__TIMEOUT_RETRIES__", (Escape-BashSingleQuoted $ObligationTimeoutRetries.ToString())).
            Replace("__TIMEOUT_MULTIPLIER__", (Escape-BashSingleQuoted $ObligationTimeoutMultiplier.ToString([System.Globalization.CultureInfo]::InvariantCulture))).
            Replace("__MEM_SOFT__", (Escape-BashSingleQuoted $MemorySoftUsedPercent.ToString())).
            Replace("__MEM_HARD__", (Escape-BashSingleQuoted $MemoryHardUsedPercent.ToString())).
            Replace("__HARD_ACTION__", (Escape-BashSingleQuoted $HardPressureAction)).
            Replace("__JOB__", (Escape-BashSingleQuoted $DispatchJobName))
        $out = Invoke-RemoteScript $startScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "StopDeferred" {
        $stopTemplate = @'
set -uo pipefail
source "__BASE__/bin/env.sh"
mode='__STOPMODE__'
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

is_lane_running() {
  local v="$1"
  pgrep -f "__BASE__/bin/run_deferred_lane.sh ${v} " >/dev/null 2>&1
}

is_dispatch_running() {
  pgrep -f "__BASE__/bin/dispatch_deferred_lanes.sh" >/dev/null 2>&1
}

if [[ "$mode" == "all" ]]; then
  mapfile -t pids < <(pgrep -f "__BASE__/bin/run_deferred_lane.sh|__BASE__/bin/dispatch_deferred_lanes.sh|__BASE__/tools/bin/cargo-kani|cbmc .*__BASE__/work/targets/" || true)
  if [[ ${#pids[@]} -gt 0 ]]; then
    for p in "${pids[@]}"; do kill -TERM "$p" 2>/dev/null || true; done
    sleep 3
    for p in "${pids[@]}"; do
      if kill -0 "$p" 2>/dev/null; then
        kill -KILL "$p" 2>/dev/null || true
      fi
    done
  fi
fi

marked_jobs=0
for d in "__BASE__"/state/jobs/*; do
  [[ -d "$d" ]] || continue
  [[ -f "$d/exit_code" ]] && continue
  cmd="$(cat "$d/command.txt" 2>/dev/null || true)"
  [[ "$cmd" == *"dispatch_deferred_lanes.sh"* || "$cmd" == *"run_deferred_lane.sh"* ]] || continue

  pid="$(cat "$d/pid" 2>/dev/null || true)"
  alive=0
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then alive=1; fi

  if [[ "$cmd" == *"dispatch_deferred_lanes.sh"* ]]; then
    if [[ "$mode" == "stale" ]]; then
      lane_pid="$(sed -nE 's/.*started lane=.* pid=([0-9]+).*/\1/p' "$d/run.log" 2>/dev/null | tail -n 1 || true)"
      if [[ -n "$lane_pid" ]] && kill -0 "$lane_pid" 2>/dev/null; then
        continue
      fi
      if (( alive == 1 )); then
        continue
      fi
    fi
  fi

  if [[ "$cmd" == *"run_deferred_lane.sh"* ]]; then
    v="$(echo "$cmd" | sed -nE 's/.*run_deferred_lane\.sh ([0-9]+).*/\1/p' | head -n 1)"
    if [[ -n "$v" ]] && is_lane_running "$v" && [[ "$mode" == "stale" ]]; then
      continue
    fi
    if (( alive == 1 )) && [[ "$mode" == "stale" ]]; then
      continue
    fi
  fi

  echo "143" > "$d/exit_code"
  printf '%s\n' "end=$ts" >> "$d/meta"
  printf '%s\n' "stop_reason=manual-$mode-reconcile" >> "$d/meta"
  marked_jobs=$((marked_jobs + 1))
done

marked_lanes=0
for d in "__BASE__"/state/deferred_lanes/*; do
  [[ -d "$d" ]] || continue
  [[ -f "$d/exit_code" ]] && continue
  lane="$(basename "$d")"
  version="$(echo "$lane" | sed -nE 's/^v([0-9]+)-.*/\1/p')"
  if [[ -n "$version" ]] && is_lane_running "$version" && [[ "$mode" == "stale" ]]; then
    continue
  fi
  echo "143" > "$d/exit_code"
  echo "completed:stopped" > "$d/status.txt"
  printf '%s\n' "status=stopped" > "$d/summary.txt"
  printf '%s\n' "timestamp_utc=$ts" >> "$d/summary.txt"
  printf '%s\n' "reason=manual-$mode-reconcile" >> "$d/summary.txt"
  marked_lanes=$((marked_lanes + 1))
done

marked_dispatch=0
for d in "__BASE__"/state/deferred_dispatch/*; do
  [[ -d "$d" ]] || continue
  state="$(cat "$d/state.txt" 2>/dev/null || true)"
  if [[ "$state" == completed:* ]]; then
    continue
  fi
  if is_dispatch_running && [[ "$mode" == "stale" ]]; then
    continue
  fi
  echo "completed:stopped" > "$d/state.txt"
  if ! grep -q '^finished_utc=' "$d/meta.env" 2>/dev/null; then
    echo "finished_utc=$ts" >> "$d/meta.env"
  fi
  if ! grep -q '^failures=' "$d/meta.env" 2>/dev/null; then
    echo "failures=1" >> "$d/meta.env"
  fi
  marked_dispatch=$((marked_dispatch + 1))
done

echo "stop_mode=$mode"
echo "marked_jobs=$marked_jobs"
echo "marked_lanes=$marked_lanes"
echo "marked_dispatch=$marked_dispatch"
echo "remaining_active:"
pgrep -af "__BASE__/bin/run_deferred_lane.sh|__BASE__/bin/dispatch_deferred_lanes.sh|__BASE__/tools/bin/cargo-kani|cbmc .*__BASE__/work/targets/" || true
exit 0
'@
        $stopScript = $stopTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__STOPMODE__", (Escape-BashSingleQuoted $StopMode))
        $out = Invoke-RemoteScript $stopScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "Status" {
        $statusTemplate = @'
set -uo pipefail
source "__BASE__/bin/env.sh"
"__BASE__/bin/list_jobs.sh"
echo "---"
"__BASE__/bin/deferred_status.sh" || true
echo "---"
if [[ -x "__BASE__/bin/resource_snapshot.sh" ]]; then
  "__BASE__/bin/resource_snapshot.sh" || true
fi
echo "---"
ls -1 "__BASE__/state/deferred_dispatch"/PAUSE_NEW_LANES* 2>/dev/null || echo "pause_flags=none"
echo "---"
latest_dispatch="$(find __BASE__/state/deferred_dispatch -mindepth 1 -maxdepth 1 -type d -print 2>/dev/null | sort | tail -n 1 || true)"
if [[ -n "$latest_dispatch" ]]; then
  echo "dispatch_dir=$latest_dispatch"
  cat "$latest_dispatch/meta.env" || true
  [[ -f "$latest_dispatch/state.txt" ]] && echo "dispatch_state=$(cat "$latest_dispatch/state.txt")"
fi
exit 0
'@
        $statusScript = $statusTemplate.Replace("__BASE__", $RemoteBase)
        $out = Invoke-RemoteScript $statusScript
        $out | ForEach-Object { Write-Host $_ }
    }
    "Monitor" {
        $monitorTemplate = @'
set -uo pipefail
source "__BASE__/bin/env.sh"
soft='__MEM_SOFT__'
hard='__MEM_HARD__'
hard_action='__HARD_ACTION__'
auto_resume='__AUTO_RESUME__'
pause_file="__BASE__/state/deferred_dispatch/PAUSE_NEW_LANES.auto"
manual_pause_file="__BASE__/state/deferred_dispatch/PAUSE_NEW_LANES.manual"

if [[ ! -x "__BASE__/bin/resource_snapshot.sh" ]]; then
  echo "resource_snapshot=missing"
  exit 0
fi

snapshot="$("__BASE__/bin/resource_snapshot.sh")"
printf '%s\n' "$snapshot"
used="$(printf '%s\n' "$snapshot" | awk -F= '$1=="mem_used_percent"{print $2}' | tail -n 1)"
if [[ -z "$used" ]]; then
  used=0
fi

if (( used >= hard )); then
  case "$hard_action" in
    halt-one)
      pid="$(ps -eo pid,rss,args --sort=-rss | awk '/cargo-kani|cbmc/ && $0 !~ /awk/ {print $1; exit}')"
      if [[ -n "${pid:-}" ]]; then
        kill -TERM "$pid" 2>/dev/null || true
        sleep 2
        if kill -0 "$pid" 2>/dev/null; then
          kill -KILL "$pid" 2>/dev/null || true
        fi
        echo "monitor_action=halt-one pid=$pid mem_used_percent=$used hard=$hard"
      else
        echo "monitor_action=halt-one pid=none mem_used_percent=$used hard=$hard"
      fi
      ;;
    halt-all)
      mapfile -t pids < <(pgrep -f "__BASE__/tools/bin/cargo-kani|cbmc .*__BASE__/work/targets/" || true)
      for p in "${pids[@]:-}"; do
        kill -TERM "$p" 2>/dev/null || true
      done
      sleep 2
      for p in "${pids[@]:-}"; do
        if kill -0 "$p" 2>/dev/null; then
          kill -KILL "$p" 2>/dev/null || true
        fi
      done
      echo "monitor_action=halt-all count=${#pids[@]} mem_used_percent=$used hard=$hard"
      ;;
    pause)
      touch "$pause_file"
      echo "monitor_action=pause mem_used_percent=$used hard=$hard"
      ;;
    none|*)
      echo "monitor_action=none mem_used_percent=$used hard=$hard"
      ;;
  esac
elif (( used >= soft )); then
  touch "$pause_file"
  echo "monitor_action=soft-pause mem_used_percent=$used soft=$soft"
else
  if [[ "$auto_resume" == "1" && -f "$pause_file" ]]; then
    rm -f "$pause_file"
    echo "monitor_action=resume mem_used_percent=$used soft=$soft"
  fi
fi

if [[ -f "$pause_file" ]]; then
  echo "pause_flag_auto=present"
else
  echo "pause_flag_auto=absent"
fi
if [[ -f "$manual_pause_file" ]]; then
  echo "pause_flag_manual=present"
else
  echo "pause_flag_manual=absent"
fi
exit 0
'@
        $autoResumeToken = if ($MonitorAutoResume) { "1" } else { "0" }
        $monitorScriptBase = $monitorTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__MEM_SOFT__", (Escape-BashSingleQuoted $MemorySoftUsedPercent.ToString())).
            Replace("__MEM_HARD__", (Escape-BashSingleQuoted $MemoryHardUsedPercent.ToString())).
            Replace("__HARD_ACTION__", (Escape-BashSingleQuoted $HardPressureAction)).
            Replace("__AUTO_RESUME__", $autoResumeToken)

        $iterations = 1
        if ($MonitorDurationSeconds -gt 0) {
            $iterations = [Math]::Max(1, [int][Math]::Ceiling($MonitorDurationSeconds / [double]$MonitorIntervalSeconds))
        }

        for ($i = 1; $i -le $iterations; $i++) {
            Write-Host ("monitor_sample={0}/{1}" -f $i, $iterations)
            $out = Invoke-RemoteScript $monitorScriptBase
            $out | ForEach-Object { Write-Host $_ }
            if ($i -lt $iterations) {
                Start-Sleep -Seconds $MonitorIntervalSeconds
            }
        }
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
if [[ -f "__BASE__/state/deferred_lanes/__LANE__/driver.log" ]]; then
  echo "lane_driver=__LANE__"
  tail -n __TAILLINES__ "__BASE__/state/deferred_lanes/__LANE__/driver.log"
fi
if [[ -f "__BASE__/state/deferred_lanes/__LANE__/summary.txt" ]]; then
  echo "lane_summary=__LANE__"
  cat "__BASE__/state/deferred_lanes/__LANE__/summary.txt"
fi
if [[ -f "__BASE__/state/deferred_lanes/__LANE__/progress.json" ]]; then
  echo "lane_progress=__LANE__"
  cat "__BASE__/state/deferred_lanes/__LANE__/progress.json"
fi
'@
        }
        $laneClause = $laneClauseTemplate.
            Replace("__BASE__", $RemoteBase).
            Replace("__LANE__", $Lane.Replace("'", "")).
            Replace("__TAILLINES__", $TailLines.ToString())

        $tailTemplate = @'
set -uo pipefail
source "__BASE__/bin/env.sh"
latest_dispatch="$(find __BASE__/state/deferred_dispatch -mindepth 1 -maxdepth 1 -type d -print 2>/dev/null | sort | tail -n 1 || true)"
if [[ -n "$latest_dispatch" ]]; then
  echo "dispatch_dir=$latest_dispatch"
  if [[ -f "$latest_dispatch/dispatch.log" ]]; then
    tail -n __TAILLINES__ "$latest_dispatch/dispatch.log"
  fi
fi
$__LANECLAUSE__
exit 0
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
        & scp @SshCommonOptions -i $SshKeyPath "$SshUser@$SshHost`:$remoteBundle" $localBundle
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to fetch remote bundle: $remoteBundle"
        }
        Write-Host "fetched: $localBundle"
    }
}
