set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
    @just --list

info:
    @echo "Iris Chat commands"
    @echo
    @echo "Run"
    @echo "  just run-android"
    @echo "  just run-ios"
    @echo "  just run-macos"
    @echo
    @echo "Bindings and native builds"
    @echo "  just gen-kotlin"
    @echo "  just android-rust"
    @echo "  just android-assemble"
    @echo "  just android-beta-apk"
    @echo "  just android-release-bundle"
    @echo "  just ios-gen-swift"
    @echo "  just ios-rust"
    @echo "  just ios-xcframework"
    @echo "  just ios-xcodeproj"
    @echo "  just macos-gen-swift"
    @echo "  just macos-rust"
    @echo "  just macos-xcframework"
    @echo "  just macos-xcodeproj"
    @echo "  just macos-build"
    @echo "  just ios-release-prepare"
    @echo "  just ios-release-archive"
    @echo
    @echo "Checks"
    @echo "  just doctor-ios"
    @echo "  just qa"
    @echo "  just qa-native-contract"
    @echo "  just qa-interop"

run-ios:
    ./tools/run-ios

run-macos:
    ./tools/run-macos

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

macos-gen-swift:
    ./scripts/macos-build macos-gen-swift

macos-rust:
    ./scripts/macos-build macos-rust

macos-xcframework:
    ./scripts/macos-build macos-xcframework

macos-xcodeproj:
    ./scripts/macos-build macos-xcodeproj

macos-build:
    ./scripts/macos-build macos-build

android-rust:
    ./scripts/android-build android-rust

gen-kotlin:
    ./scripts/android-build gen-kotlin

android-assemble:
    ./scripts/android-build android-assemble

android-beta-apk:
    ./scripts/android-release beta-apk

android-release-bundle:
    ./scripts/android-release release-bundle

ios-release-prepare:
    ./scripts/ios-release prepare

ios-release-archive:
    ./scripts/ios-release archive

doctor-ios:
    ./tools/ios-runtime-doctor

qa:
    ./scripts/test_fast.sh

qa-native-contract:
    ./scripts/test_native_contract.sh

qa-interop:
    ./scripts/test_interop_confidence.sh
