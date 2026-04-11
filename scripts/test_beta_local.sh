#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"${ROOT_DIR}/scripts/test_rust.sh"

cd "${ROOT_DIR}/android"
./gradlew :app:compileDebugKotlin :app:compileBetaKotlin :app:compileDebugAndroidTestKotlin
