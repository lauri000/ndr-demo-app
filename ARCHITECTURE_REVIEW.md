# Architecture Review

Rust-first review of the Iris Chat app as of 2026-04-20.

This review is calibrated against the intended architecture goal:

- Rust owns as much product behavior as possible.
- Android and iOS are thin native shells.
- Navigation, app state, and business logic stay Rust-owned by default.
- Native code keeps rendering concerns, secure storage, and true platform bridges.

## Executive Summary

The repo is now aligned with the intended Rust-first direction.

The biggest change since the earlier review is that the shell boundary is no
longer implicit:

- the ownership model is documented
- Android and iOS have explicit shell-contract tests
- the blocking refactor gate now exercises Rust, iOS XCTest, Android shell
  contract tests, Android smoke tests, and the local relay soak
- Android no longer fabricates an authoritative logged-out shell snapshot during
  reset/logout

That means the primary remaining issue is not boundary confusion. It is
maintainability inside the Rust core.

## What Is Solid Now

### 1. Rust is clearly the source of truth

The repo already follows the intended flow:

- native dispatches `AppAction`
- Rust mutates internal state
- Rust emits `AppUpdate`
- native renders `AppState`

Relevant files:

- `core/src/lib.rs`
- `core/src/actions.rs`
- `core/src/state.rs`
- `core/src/updates.rs`

### 2. Rust-owned routing is the correct model here

`AppState.router` plus Rust-owned `Screen` values are aligned with the intended
architecture. Native shells render the route. They do not own alternate
navigation state.

Relevant files:

- `core/src/state.rs`
- `core/src/actions.rs`
- `core/src/core.rs`

### 3. The shell contract is now explicit and tested

The contract is enforced in both shells:

- restore secure credentials into Rust
- persist secure side effects even when revs race
- drop stale full-state snapshots
- keep logout/reset ownership in Rust

Relevant files:

- `android/app/src/main/java/social/innode/ndr/demo/core/AppManager.kt`
- `android/app/src/androidTest/java/social/innode/ndr/demo/core/AppManagerContractTest.kt`
- `ios/Sources/AppManager.swift`
- `ios/Tests/IrisChatTests.swift`

### 4. The repo now has a useful gate split

The test lanes match the current architecture work:

- `just qa`
  - fast local confidence
- `just qa-native-contract`
  - blocking gate before Rust refactors
- `just qa-interop`
  - heavier relay-backed confidence lane

This is the right shape for a Rust-core refactor.

## Open Findings

### 1. `core/src/core.rs` is still the main architectural hotspot

This is the biggest remaining issue.

`core/src/core.rs` still mixes:

- action handling
- restore/bootstrap
- direct-chat behavior
- group behavior
- device authorization behavior
- subscription planning and catch-up
- persistence
- `AppState` projection
- support/debug logic
- large in-file tests

This does not violate the Rust-first goal. It makes the Rust-first goal harder
to maintain.

### 2. Domain state and projection are still tightly interleaved

The repo uses Rust-owned snapshots correctly, but source state, persistence, and
UI projection are still too interwoven.

The first extractions should target:

- `storage`
- `state_projection`
- `subscriptions`

Those splits reduce coupling without changing the public UniFFI contract.

### 3. Full-state snapshots remain acceptable, but they are still a tradeoff

`AppUpdate::FullState(AppState)` is still the correct default. It is coherent
and easy to reason about.

It is also the next obvious place to look if:

- histories get much larger
- FFI transfer cost becomes measurable
- snapshot cloning starts to dominate UI latency

This is not a current design flaw. It is a future measurement point.

### 4. Acceptance coverage is better, but not complete

The repo now has:

- local Android smoke tests
- local iOS smoke tests
- Android shell contract tests
- iOS shell contract tests
- mixed Android+iOS interop scripts

What remains thinner than the core coverage:

- dedicated iOS<->iOS relay-backed acceptance
- iOS restore-history convergence coverage
- broader real-device acceptance beyond local emulator/simulator lanes

## Recommended Next Step

Start internal Rust modularization without changing the public FFI boundary.

The first extraction should be:

1. `storage`
2. `state_projection`

Why this first:

- high readability payoff
- lower user-facing risk than session/outbox rewrites
- it keeps the Rust-first architecture intact
- it makes later `session`, `threads_outbox`, and `groups` extractions easier

## Bottom Line

The repo no longer has a boundary-definition problem.

It has a Rust-core modularity problem, which is the right problem to have for
this architecture. The next work should improve the internal shape of the Rust
core while keeping the cross-platform ownership model stable.
