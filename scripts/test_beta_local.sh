#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_LIB_ROOT="$(cd "${ROOT_DIR}/../nostr-double-ratchet" && pwd)"
LIB_ROOT="${NDR_LIB_ROOT:-${DEFAULT_LIB_ROOT}}"

if [[ ! -x "${LIB_ROOT}/scripts/test_rust.sh" ]]; then
  echo "Library test runner not found at ${LIB_ROOT}/scripts/test_rust.sh" >&2
  exit 1
fi

"${LIB_ROOT}/scripts/test_rust.sh"
"${ROOT_DIR}/scripts/test_rust.sh"

cd "${ROOT_DIR}"
./gradlew :app:compileDebugKotlin :app:compileBetaKotlin :app:compileDebugAndroidTestKotlin
