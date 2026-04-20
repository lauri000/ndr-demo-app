# Architecture Implementation Plan

Implementation plan derived from [ARCHITECTURE.md](ARCHITECTURE.md) and
[ARCHITECTURE_REVIEW.md](ARCHITECTURE_REVIEW.md).

## Goal

Keep the current product direction:

- Rust owns navigation, app state, and business logic
- Android and iOS remain thin shells

Improve the implementation so that:

- the Rust core is internally modular
- the shell contract stays explicit and tested
- iOS and Android shell behavior does not drift
- another future shell can be added without re-implementing app behavior

## Current Status

### Completed in the pre-refactor hardening phase

- `ARCHITECTURE.md` now describes the Rust-first ownership model directly.
- Android and iOS `AppManager` code now has contract comments around restore,
  side-effect persistence, stale update dropping, and logout/reset ownership.
- Android shell contract tests exist in
  `android/app/src/androidTest/java/social/innode/ndr/demo/core/AppManagerContractTest.kt`.
- iOS shell contract tests exist in `ios/Tests/IrisChatTests.swift`.
- Android no longer fabricates an authoritative logged-out snapshot during
  reset/logout.
- The repo now has explicit refactor gates:
  - `just qa`
  - `just qa-native-contract`
  - `just qa-interop`

### Not started yet

- internal modularization of `core/src/core.rs`
- clearer separation of persisted source state from `AppState` projection
- broader relay-backed iOS acceptance coverage

## Non-Goals

This plan does not aim to:

- move routing into native code
- replace full-state snapshots with granular deltas immediately
- redesign the public UniFFI contract without evidence
- duplicate business logic in native apps

## Invariants To Preserve

The following should stay true throughout the refactor:

- `AppAction` remains the fire-and-forget input surface
- `AppState` remains the Rust-owned render model
- `Router` remains Rust-owned
- native shells restore secrets and persist secure side effects
- protocol and domain logic stay in Rust
- mixed-platform behavior remains acceptance-testable through shared-core flows

## Next Workstreams

### Workstream 1: Split `core/src/core.rs` internally

This is the highest-priority remaining work.

Keep the external FFI stable while breaking the implementation into smaller
modules behind it.

Recommended extraction order:

1. `storage`
2. `state_projection`
3. `subscriptions`
4. `session`
5. `threads_outbox`
6. `groups`
7. `support_debug`

Rules:

- preserve behavior first
- avoid redesigning the public contract and internal structure in the same pass
- move tests with the extracted responsibility where practical

### Workstream 2: Separate domain state from projection more clearly

Clarify which fields are:

- actor-internal source state
- durable persisted state
- derived projection for `AppState`

This work should land naturally while extracting `storage` and
`state_projection`.

### Workstream 3: Keep snapshot simplicity until data says otherwise

Continue using:

- `AppUpdate::FullState`
- secure side-effect updates for credential persistence

Only revisit granular updates if:

- large histories measurably hurt performance
- FFI transfer or cloning is shown to be a bottleneck
- snapshot persistence/update cost becomes unacceptable

### Workstream 4: Expand heavy confidence coverage gradually

The blocking gate is in place. The heavier lane should continue to grow as the
Rust core is split.

Keep exercising:

- `scripts/mixed_platform_group_chat_matrix.sh`
- `scripts/group_chat_restore_smoke.sh`
- `scripts/linked_device_relay_matrix.sh`

Broaden coverage later for:

- iOS<->iOS relay-backed acceptance
- iOS restore-history convergence
- more real-device acceptance

## Execution Order

### Phase 1: Completed

- finalize architecture docs
- add Android shell contract tests
- add iOS shell contract tests
- remove the most obvious native policy drift
- establish the blocking and heavy confidence lanes

### Phase 2: Core shape cleanup

- extract `storage`
- extract `state_projection`
- extract `subscriptions`

### Phase 3: Messaging runtime modularization

- extract `session`
- extract `threads_outbox`
- extract `groups`

### Phase 4: Cleanup and tightening

- extract `support_debug`
- clean remaining shell drift if any reappears
- update docs after each boundary change
- decide whether any snapshot/performance work is actually needed

## Recommended First Refactor

The first code refactor after this hardening phase should be:

- extract Rust persistence and state projection out of `core/src/core.rs`

Why first:

- high readability payoff
- lower user-facing risk than transport/session changes
- keeps the public model stable
- makes later runtime extractions easier

## Refactor Gate

Before and during the Rust-core split, the minimum blocking gate is:

```bash
just qa-native-contract
```

Use the heavier lane at milestone boundaries:

```bash
just qa-interop
```

## Done Criteria

This plan is complete when:

- Rust-first ownership is documented and enforced consistently
- Android and iOS shells remain contract-tested
- native code does not own correctness policy
- `core/src/core.rs` is no longer the single implementation hotspot
- adding another shell would primarily mean rendering and platform integration,
  not re-implementing app behavior
