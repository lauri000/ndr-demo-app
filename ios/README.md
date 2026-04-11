# iOS

This directory contains the native iOS UI shell.

Planned shape:

- SwiftUI app shell
- UniFFI-generated Swift bindings from `../core`
- XCFramework packaging for `ndr_demo_core`
- simulator smoke and upgrade test entrypoints in `../scripts`

The goal is one shared Rust app core with thin native Android and iOS UIs above it.
