# Parity Matrix

Status tracking for the shared `ndr-demo-app` workspace.

Legend:

- `Done`: implemented and verified locally
- `Partial`: implemented but not fully acceptance-tested yet
- `Planned`: not implemented yet

| Workflow | Rust | Android | iOS | Acceptance |
| --- | --- | --- | --- | --- |
| Create owner account | Done | Done | Done | Partial |
| Restore from owner `nsec` | Done | Done | Done | Partial |
| Start linked device from owner QR/paste | Done | Done | Done | Partial |
| Approve pending linked device | Done | Done | Done | Partial |
| Remove authorized device | Done | Done | Done | Partial |
| Device revoked screen | Done | Done | Done | Partial |
| Chat list routing | Done | Done | Done | Done |
| Create direct chat | Done | Done | Done | Partial |
| Send direct message | Done | Done | Done | Partial |
| Create group | Done | Done | Done | Partial |
| Rename group | Done | Done | Done | Partial |
| Add group members | Done | Done | Done | Partial |
| Remove group members | Done | Done | Done | Partial |
| Group details screen | Done | Done | Done | Partial |
| Profile sheet | Done | Done | Done | Partial |
| Owner QR display | Done | Done | Done | Partial |
| Support bundle export/copy | Done | Done | Done | Partial |
| Shared device-approval QR codec | Done | Done | Done | Done |
| Android run tooling from repo root | n/a | Done | n/a | Done |
| iOS run tooling from repo root | n/a | n/a | Done | Done |
| Root repo self-contained build | Done | Done | Done | Done |
| iOS Keychain store tests | Planned | n/a | Planned | Planned |
| iOS AppManager reconcile tests | Planned | n/a | Planned | Planned |
| Android/iOS interop smoke matrix | Done | Partial | Partial | Partial |
| iOS <-> iOS chat acceptance | Done | n/a | Planned | Planned |
| iOS <-> Android chat acceptance | Done | Partial | Planned | Planned |
| Restore history convergence on iOS | Done | n/a | Planned | Planned |

## Current Notes

- The repo is now structured as `core/`, `android/`, and `ios/`, with the protocol crates vendored under `core/vendor/`.
- Android and iOS both consume the same UniFFI surface from `core/`.
- The device approval QR format is now owned by `core/` and generated into both native clients.
- iOS builds and tests succeed on the local simulator.
- Android launches from the root tooling and compiles cleanly from the new layout.
- The iOS Rust archive still emits linker warnings about simulator deployment version skew. The build succeeds, but the target flags should be tightened further.
