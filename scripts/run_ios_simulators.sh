#!/usr/bin/env bash

set -Eeuo pipefail

DEFAULT_SIMULATORS=("NDR Demo iPhone" "NDR Demo iPhone 2")
LIST_ONLY=0
NO_OPEN=0
SIMULATORS=()

usage() {
  cat <<'EOF'
Usage: scripts/run_ios_simulators.sh [options] [simulator-name...]

Options:
  --list     Print available simulators and runtimes, then exit
  --no-open  Do not open the Simulator app after booting

Defaults:
  NDR Demo iPhone
  NDR Demo iPhone 2
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --list)
      LIST_ONLY=1
      shift
      ;;
    --no-open)
      NO_OPEN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      SIMULATORS+=("$1")
      shift
      ;;
  esac
done

if ! command -v xcrun >/dev/null 2>&1; then
  echo "xcrun not found. Install Xcode command line tools." >&2
  exit 1
fi

if [[ ${LIST_ONLY} -eq 1 ]]; then
  xcrun simctl list runtimes available
  echo "---"
  xcrun simctl list devices available
  exit 0
fi

if [[ ${#SIMULATORS[@]} -eq 0 ]]; then
  SIMULATORS=("${DEFAULT_SIMULATORS[@]}")
fi

SETUP_RAW="$(
  xcrun simctl list -j devicetypes runtimes devices | python3 -c '
import json
import re
import sys

data = json.load(sys.stdin)
runtimes = [
    runtime for runtime in data.get("runtimes", [])
    if runtime.get("isAvailable") and runtime.get("identifier", "").startswith("com.apple.CoreSimulator.SimRuntime.iOS")
]
if not runtimes:
    raise SystemExit("NO_IOS_RUNTIME")

def version_key(runtime):
    identifier = runtime.get("identifier", "")
    match = re.search(r"iOS[- ](.+)$", runtime.get("name", "")) or re.search(r"iOS-(.+)$", identifier)
    parts = re.findall(r"\d+", match.group(1) if match else "")
    return tuple(int(part) for part in parts) if parts else (0,)

runtime = max(runtimes, key=version_key)
preferred = ["iPhone 16", "iPhone 16 Pro", "iPhone 15", "iPhone 14"]
device_types = data.get("devicetypes", [])
device_type = None
for name in preferred:
    device_type = next((item for item in device_types if item.get("name") == name), None)
    if device_type is not None:
        break
if device_type is None:
    device_type = next((item for item in device_types if "iPhone" in item.get("name", "")), None)
if device_type is None:
    raise SystemExit("NO_IPHONE_DEVICE_TYPE")

print(runtime["identifier"])
print(device_type["identifier"])
print(device_type["name"])
'
)"

SETUP=()
while IFS= read -r line; do
  SETUP+=("${line}")
done <<< "${SETUP_RAW}"

if [[ ${#SETUP[@]} -lt 3 ]]; then
  echo "No available iOS simulator runtime found. Install an iOS runtime in Xcode Settings > Components and try again." >&2
  exit 1
fi

RUNTIME_ID="${SETUP[0]}"
DEVICE_TYPE_ID="${SETUP[1]}"
DEVICE_TYPE_NAME="${SETUP[2]}"

find_device_udid() {
  local simulator_name="$1"
  xcrun simctl list -j devices | python3 -c '
import json
import sys

runtime_id = sys.argv[1]
name = sys.argv[2]
data = json.load(sys.stdin)
for device in data.get("devices", {}).get(runtime_id, []):
    if device.get("name") == name:
        print(device.get("udid", ""))
        break
' "${RUNTIME_ID}" "${simulator_name}"
}

for simulator_name in "${SIMULATORS[@]}"; do
  udid="$(find_device_udid "${simulator_name}")"
  if [[ -z "${udid}" ]]; then
    udid="$(xcrun simctl create "${simulator_name}" "${DEVICE_TYPE_ID}" "${RUNTIME_ID}")"
  fi

  xcrun simctl boot "${udid}" >/dev/null 2>&1 || true
  xcrun simctl bootstatus "${udid}" -b >/dev/null
  echo "${simulator_name} ${udid} ${DEVICE_TYPE_NAME} ${RUNTIME_ID}"
done

if [[ ${NO_OPEN} -eq 0 ]]; then
  open -a Simulator >/dev/null 2>&1 || true
fi
