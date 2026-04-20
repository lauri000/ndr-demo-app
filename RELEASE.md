# Release Guide

This repo now has repeatable release entrypoints for both platforms:

- Android: `./scripts/android-release`
- iOS: `./scripts/ios-release`
- Shared release inputs: `release.env` copied from [release.env.example](/Users/l/Projects/iris-fork/ndr-demo-app/release.env.example)

## Official References

- Apple upload/TestFlight workflow:
  - [Upload builds](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/)
  - [Add a new app](https://developer.apple.com/help/app-store-connect/create-an-app-record/add-a-new-app/)
  - [TestFlight overview](https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview)
  - [Distributing your app for beta testing and releases](https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases/)
- Google Play release workflow:
  - [Prepare your app for release](https://developer.android.com/studio/publish/preparing)
  - [Sign your app](https://developer.android.com/guide/publishing/app-signing.html)
  - [Publish your app](https://developer.android.com/studio/publish)
  - [Upload your app to the Play Console](https://developer.android.com/studio/publish/upload-bundle)

## Repo Layout

- `core/`: shared Rust core. Mobile build metadata and default relay sets are compiled here via `core/build.rs`.
- `android/`: Gradle/Compose shell. Android package metadata, signing config, and Rust Android packaging are controlled from `android/app/build.gradle.kts`.
- `ios/`: SwiftUI shell. The Xcode project is generated from `ios/project.yml`, while version/build values come from Xcode build settings referenced by `ios/Info.plist`.
- `scripts/`: release/test/build entrypoints.

## Shared Build Inputs

These values are the common boundary between Android, iOS, and the Rust core:

- `NDR_APP_VERSION_NAME`
- `NDR_APP_VERSION_CODE`
- `NDR_BUILD_GIT_SHA`
- `NDR_BUILD_TIMESTAMP_UTC`
- `NDR_RELEASE_RELAY_SET_ID`
- `NDR_RELEASE_RELAYS`

If you leave `NDR_BUILD_GIT_SHA` and `NDR_BUILD_TIMESTAMP_UTC` unset, the release scripts derive them from the current Git `HEAD`. If you want a stricter deterministic build, set them explicitly or provide `SOURCE_DATE_EPOCH`.

## Android Organization

Android release inputs are read in this order: `android/local.properties` then environment variables.

Supported keys:

- App version:
  - `app.versionName` or `NDR_APP_VERSION_NAME`
  - `app.versionCode` or `NDR_APP_VERSION_CODE`
- Build metadata:
  - `build.gitSha` or `NDR_BUILD_GIT_SHA`
  - `build.timestampUtc` or `NDR_BUILD_TIMESTAMP_UTC`
- Relay/channel config:
  - `debug.relaySetId` or `NDR_DEBUG_RELAY_SET_ID`
  - `debug.relays` or `NDR_DEBUG_RELAYS`
  - `beta.relaySetId` or `NDR_BETA_RELAY_SET_ID`
  - `beta.relays` or `NDR_BETA_RELAYS`
  - `release.relaySetId` or `NDR_RELEASE_RELAY_SET_ID`
  - `release.relays` or `NDR_RELEASE_RELAYS`
- Signing:
  - `beta.storeFile` or `NDR_BETA_KEYSTORE_PATH`
  - `beta.storePassword` or `NDR_BETA_KEYSTORE_PASSWORD`
  - `beta.keyAlias` or `NDR_BETA_KEY_ALIAS`
  - `beta.keyPassword` or `NDR_BETA_KEY_PASSWORD`
  - `release.storeFile` or `NDR_RELEASE_KEYSTORE_PATH`
  - `release.storePassword` or `NDR_RELEASE_KEYSTORE_PASSWORD`
  - `release.keyAlias` or `NDR_RELEASE_KEY_ALIAS`
  - `release.keyPassword` or `NDR_RELEASE_KEY_PASSWORD`

Primary commands:

- `./scripts/android-release print-config`
- `./scripts/android-release beta-apk`
- `./scripts/android-release beta-bundle`
- `./scripts/android-release release-bundle`

Artifacts are copied into `dist/android/` with a stable `IrisChat-<channel>-<version>+<build>-<sha>` naming scheme and a matching `.env` manifest.

## iOS Organization

iOS has two layers:

- `./scripts/ios-build`: native build primitives
  - generate Swift bindings
  - build Rust static libs/XCFramework
  - generate the Xcode project
  - run simulator builds/tests
- `./scripts/ios-release`: release orchestration
  - `prepare`
  - `archive`
  - `export`
  - `upload`

iOS release environment:

- `NDR_IOS_BUNDLE_ID`
- `NDR_IOS_DEVELOPMENT_TEAM`
- `NDR_IOS_SIGNING_STYLE`
- `NDR_IOS_EXPORT_METHOD`
- `NDR_IOS_INTERNAL_ONLY`
- `NDR_IOS_ALLOW_PROVISIONING_UPDATES`
- `NDR_ASC_AUTH_KEY_PATH`
- `NDR_ASC_AUTH_KEY_ID`
- `NDR_ASC_AUTH_KEY_ISSUER_ID`

The generated project now takes its version/build from `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION`, so `ios-release` can archive the same source tree with explicit release values instead of editing plist files in place.

## Step By Step

### Android closed test or release

1. Copy `release.env.example` to `release.env`.
2. Fill `NDR_APP_VERSION_NAME`, `NDR_APP_VERSION_CODE`, relay values, and release signing values.
3. Build a Play artifact:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/android-release release-bundle
```

4. Upload the `.aab` from `dist/android/` to the correct track in Play Console.
5. For a side-loadable trusted beta, use `./scripts/android-release beta-apk`.

### iOS TestFlight

1. In App Store Connect, create the app record first. Apple requires the app record before the first upload.
2. Copy `release.env.example` to `release.env`.
3. Fill `NDR_APP_VERSION_NAME`, `NDR_APP_VERSION_CODE`, relay values, `NDR_IOS_BUNDLE_ID`, and `NDR_IOS_DEVELOPMENT_TEAM`.
4. If you want Xcode to create/fetch signing assets, set `NDR_IOS_ALLOW_PROVISIONING_UPDATES=true`.
5. Build the archive:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/ios-release archive
```

6. Export an IPA if you want a local artifact:

```bash
./scripts/ios-release export
```

7. Upload either from Xcode Organizer or with:

```bash
./scripts/ios-release upload
```

8. Wait for App Store Connect processing, then add the build to internal or external TestFlight groups.

## Current Limits

- `ios-release` automates automatic signing only. Manual provisioning profile mapping is not encoded in the script.
- App Store Connect metadata, tester groups, screenshots, privacy questionnaires, and review submissions still happen in Apple/Google consoles.
- The scripts make the build inputs explicit and repeatable. They do not guarantee bit-for-bit identical output across different toolchain versions or machines. To tighten that further, keep Xcode, Gradle, NDK, JDK, and Cargo inputs pinned and build from a clean commit with explicit metadata.
