# ndr-demo-app

Shared mobile app workspace for Nostr Double Ratchet.

Current shape:

- `core/`: shared Rust app core consumed by native UIs
- `android/`: Android UI and Gradle project
- `ios/`: iOS SwiftUI shell, bindings, and XcodeGen project spec
- `scripts/`: local tooling for bootstrap, emulators, simulators, and test gates
- `tools/`: high-signal run/doctor entrypoints

The Rust app core already uses UniFFI and is the intended single integration surface for both
Android and iOS.

Tracking:

- parity status: [PARITY_MATRIX.md](/Users/l/Projects/iris-fork/ndr-demo-app/PARITY_MATRIX.md)

## Get Started

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/mobile_bootstrap_macos.sh
just info
just run-android
just run-ios
just qa
```

## Android

Build and install the Android debug app:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
just android-assemble
./scripts/emulator_smoke.sh --clear emulator-5554 emulator-5556 emulator-5558
```

Build the shareable beta APK:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
(cd android && ./gradlew :app:assembleBeta)
```

## iOS

The iOS app is generated from `ios/project.yml` and consumes the same `core/` Rust crate via
generated Swift UniFFI bindings plus an XCFramework packaging step.

Common flows:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
just ios-gen-swift
just ios-xcframework
just ios-xcodeproj
just run-ios
```
