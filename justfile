set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
    @just --list

info:
    @echo "ndr-demo-app commands"
    @echo
    @echo "Run"
    @echo "  just run-android"
    @echo "  just run-ios"
    @echo
    @echo "Bindings and native builds"
    @echo "  just gen-kotlin"
    @echo "  just android-rust"
    @echo "  just android-assemble"
    @echo "  just ios-gen-swift"
    @echo "  just ios-rust"
    @echo "  just ios-xcframework"
    @echo "  just ios-xcodeproj"
    @echo
    @echo "Checks"
    @echo "  just doctor-ios"
    @echo "  just qa"

run-ios:
    ./tools/run-ios

run-android:
    ./tools/run-android

ios-gen-swift:
    ./scripts/ios-build ios-gen-swift

ios-rust:
    ./scripts/ios-build ios-rust

ios-xcframework:
    ./scripts/ios-build ios-xcframework

ios-xcodeproj:
    ./scripts/ios-build ios-xcodeproj

android-rust:
    ./scripts/android-build android-rust

gen-kotlin:
    ./scripts/android-build gen-kotlin

android-assemble:
    ./scripts/android-build android-assemble

doctor-ios:
    ./tools/ios-runtime-doctor

qa:
    ./scripts/test_fast.sh
