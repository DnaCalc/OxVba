#!/usr/bin/bash
set -euo pipefail

if [[ "$#" -lt 8 ]]; then
  echo "core-gate-linux-supervisor: expected ready-fd ack-fd nonce stdout-fd stderr-fd working-directory-fd executable-fd argv0 [args...]" >&2
  exit 64
fi

ready_path="$1"
ack_path="$2"
nonce="$3"
stdout_path="$4"
stderr_path="$5"
working_directory="$6"
executable_fd="$7"
executable_argv0="$8"
shift 8

exec >>"${stdout_path}" 2>>"${stderr_path}"
cd -- "${working_directory}"

# setsid execs this exact Bash process as both process-group and session leader.
# Publish its stable /proc start-time identity, then wait for the parent
# subreaper to validate and arm containment before any gate code can execute.
read -r -a stat_fields <<< "$(<"/proc/$$/stat")"
if [[ "${#stat_fields[@]}" -lt 22 ]]; then
  echo "core-gate-linux-supervisor: could not read complete /proc identity" >&2
  exit 65
fi
printf '%s|%s|%s|%s|%s\n' \
  "${nonce}" "$$" "${stat_fields[4]}" "${stat_fields[5]}" "${stat_fields[21]}" >"${ready_path}"

# The pre-ack path must remain one child-free Bash process. In particular, do
# not call sleep/mv or use pipelines, command/process substitution or
# background jobs here: root confirmation failure owns only
# the exact retained root pidfd. EPOCHREALTIME is a Bash special parameter, so
# the bounded acknowledgement poll requires no helper child.
deadline_us="${EPOCHREALTIME/./}"
deadline_us=$((10#${deadline_us} + 5000000))
while (( 10#${EPOCHREALTIME/./} < deadline_us )); do
  if [[ -f "${ack_path}" ]]; then
    ack_nonce=""
    IFS= read -r ack_nonce <"${ack_path}" || true
    if [[ "${ack_nonce}" == "${nonce}" ]]; then
      exec -a "${executable_argv0}" "${executable_fd}" "$@"
    fi
  fi
done

echo "core-gate-linux-supervisor: ownership acknowledgement timed out" >&2
exit 70
