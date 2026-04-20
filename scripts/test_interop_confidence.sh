#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"${ROOT_DIR}/scripts/mixed_platform_group_chat_matrix.sh"
"${ROOT_DIR}/scripts/group_chat_restore_smoke.sh"
"${ROOT_DIR}/scripts/linked_device_relay_matrix.sh"
