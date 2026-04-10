# iOS

This directory is reserved for the native iOS UI.

Planned shape:

- SwiftUI app shell
- UniFFI-generated Swift bindings from `../rust`
- XCFramework packaging for `ndr_demo_core`
- simulator smoke and upgrade test entrypoints in `../scripts`

The goal is one shared Rust app core with thin native Android and iOS UIs above it.
