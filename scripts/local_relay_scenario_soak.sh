#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ITERATIONS="${ITERATIONS:-100}"
RUST_DIR="${ROOT_DIR}/rust"

SCENARIOS=(
  "core::tests::twenty_owner_group_converges"
  "core::tests::group_with_linked_devices_converges"
  "core::tests::restart_mid_group_create_recovers"
  "core::tests::restart_mid_member_add_recovers"
  "core::tests::duplicate_and_replayed_events_do_not_duplicate_threads_or_messages"
  "core::tests::member_removal_blocks_future_sends"
  "core::tests::post_removal_messages_reach_only_remaining_members"
)

usage() {
  cat <<EOF
Usage: scripts/local_relay_scenario_soak.sh [--iterations N]

Runs the local-relay near-e2e app-core scenario suite repeatedly.

Options:
  --iterations N   Number of full scenario-suite passes. Default: ${ITERATIONS}
  -h, --help       Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --iterations)
      ITERATIONS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

for ((iteration = 1; iteration <= ITERATIONS; iteration++)); do
  echo "=== local relay soak iteration ${iteration}/${ITERATIONS} ==="
  for scenario in "${SCENARIOS[@]}"; do
    echo "--- ${scenario}"
    (
      cd "${RUST_DIR}" &&
        cargo test "${scenario}" -- --nocapture --test-threads=1
    )
  done
done

echo "Local relay scenario soak passed (${ITERATIONS} iterations)"
