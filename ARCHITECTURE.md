# Architecture

This file is the source of truth for the Android app architecture. Future changes should follow this split, and if implementation diverges this document must be updated in the same change.

## Repo Roles

- `/Users/l/Projects/iris-fork/nostr-double-ratchet`
  - Rust engine repo
  - `nostr-double-ratchet`: protocol/domain logic
  - `nostr-double-ratchet-nostr`: Nostr wire conversion
- `/Users/l/Projects/iris-fork/ndr-demo-android`
  - Android product repo
  - Compose UI and app-specific Rust runtime
  - `rust/`: app-facing UniFFI crate that owns the mobile app core

## Ownership Split

### Rust owns

- `FfiApp`, `AppCore`, and the UniFFI boundary used by Android
- account creation, import, and identity derivation
- app state transitions through `AppCore`
- `SessionManager`, `Session`, and `Invite`
- local chat thread and message state
- persistence blob schema and versioning
- relay subscriptions, event interpretation, and protocol decisions
- publication of rosters, invites, invite responses, and messages

### Kotlin owns

- Compose rendering and navigation
- Android lifecycle and process startup
- Android Keystore persistence for the encrypted nsec
- platform integrations like clipboard, notifications, camera, and background behavior

## Boundary

The Kotlin to Rust boundary is UniFFI.

Kotlin should not call `SessionManager` directly. Kotlin talks only to the Rust `FfiApp` facade exposed by `/Users/l/Projects/iris-fork/ndr-demo-android/rust`.

The protocol repo remains pure Rust. It contains no mobile bridge code and no UniFFI surface.

## Persistence Model

There is one persistence boundary:

- Kotlin stores the Rust secret key bytes encrypted with Android Keystore
- Rust stores the rest of the app snapshot and protocol state in app-local files
- Rust owns the format of that persisted state

Kotlin must treat the Rust app core as authoritative for runtime and persisted protocol state.

## Runtime Flow

### Startup

1. Kotlin constructs `FfiApp`.
2. Kotlin restores the encrypted nsec from Android Keystore storage.
3. Kotlin dispatches `RestoreSession` into Rust if a secret exists.
4. Rust restores account, protocol state, relay client, and chat state.
5. Kotlin renders from `AppState` updates emitted by Rust.

### Send

1. User enters text in Compose.
2. Kotlin dispatches `SendMessage`.
3. Rust mutates chat state, prepares protocol output, and publishes it to relays.
4. Rust persists the updated state.
5. Kotlin re-renders from `AppState`.

### Receive

1. Rust receives raw relay data from its own relay client.
2. Rust interprets the event and mutates protocol/chat state.
3. Rust persists the updated state.
4. Kotlin renders the resulting `AppState`.

## Mobile Rule

All mobile business logic belongs in Rust unless it is Android platform behavior.

That means Kotlin should not own:

- key generation/import rules
- chat message history
- protocol interpretation
- session bootstrap logic
- Nostr event meaning
- relay connection management for the messaging runtime

Kotlin may own:

- UI-only ephemeral state such as current text field contents
- lifecycle orchestration
- encrypted secret storage
