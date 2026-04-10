# ndr-demo-app

Shared mobile app workspace for Nostr Double Ratchet.

Current shape:

- `rust/`: shared Rust app core consumed by native UIs
- `app/`: Android UI and build
- `ios/`: future iOS UI home
- `scripts/`: local tooling for bootstrap, emulators, simulators, and test gates

The Rust app core already uses UniFFI and is the intended single integration surface for both
Android and iOS.

## Get Started

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/mobile_bootstrap_macos.sh
./scripts/run_android_emulators.sh
./scripts/run_ios_simulators.sh
./scripts/test_fast.sh
```

## Android

Build and install the Android debug app:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./gradlew :app:assembleDebug
./scripts/emulator_smoke.sh --clear emulator-5554 emulator-5556 emulator-5558
```

Build the shareable beta APK:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./gradlew :app:assembleBeta
```

## iOS

There is no Xcode project yet. The immediate next step is to add a SwiftUI app under `ios/`
consuming the same `rust/` core via UniFFI Swift bindings and an XCFramework packaging step.

Until that exists, `./scripts/run_ios_simulators.sh` is the simulator/runtime setup entrypoint.
