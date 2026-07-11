#!/usr/bin/env bash
set -euo pipefail

version="7.5.7"
expected_sha256="207a3c0b2f630e8e1226cc9beb651e2e16789f07729197f45fd3ad0902d1c593"
archive_name="powershell-${version}-linux-x64.tar.gz"
download_url="https://github.com/PowerShell/PowerShell/releases/download/v${version}/${archive_name}"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "pinned-pwsh: expected Linux x86_64" >&2
  exit 1
fi
: "${RUNNER_TEMP:?pinned-pwsh: RUNNER_TEMP is required}"
: "${GITHUB_PATH:?pinned-pwsh: GITHUB_PATH is required}"
for command_name in curl sha256sum tar; do
  command -v "${command_name}" >/dev/null 2>&1 || {
    echo "pinned-pwsh: missing required command ${command_name}" >&2
    exit 1
  }
done

tools_root="${RUNNER_TEMP}/oxvba-tools"
install_root="${tools_root}/powershell-${version}"
archive_path="${tools_root}/${archive_name}"
if [[ -e "${tools_root}" ]]; then
  echo "pinned-pwsh: owned tools root already exists; refusing retained state: ${tools_root}" >&2
  exit 1
fi
mkdir -p "${install_root}"

curl --fail --location --proto '=https' --tlsv1.2 \
  --output "${archive_path}" "${download_url}"
printf '%s  %s\n' "${expected_sha256}" "${archive_path}" | sha256sum --check --strict
tar --extract --gzip --file "${archive_path}" --directory "${install_root}"
chmod 0755 "${install_root}/pwsh"
rm --force "${archive_path}"

actual_version="$(${install_root}/pwsh -NoLogo -NoProfile -Command '$PSVersionTable.PSVersion.ToString()')"
if [[ "${actual_version}" != "${version}" ]]; then
  echo "pinned-pwsh: expected ${version}, found ${actual_version}" >&2
  exit 1
fi
printf '%s\n' "${install_root}" >> "${GITHUB_PATH}"
echo "pinned-pwsh: ok (${version})"
