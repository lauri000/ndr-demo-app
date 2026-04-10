# Android Beta Release

This app now has a dedicated `beta` build intended for a trusted closed test group.

## Beta warning

- The beta is for trusted testers only.
- Local app-core state is not encrypted at rest yet.
- Do not use the beta for sensitive conversations.

## Build configuration

The `beta` build type is release-like, optimized, and installable alongside `debug`.

Build metadata is embedded in both the app UI and the Rust support bundle:

- app version
- git SHA
- build timestamp
- relay-set ID
- build channel

## Relay configuration

The Rust core reads its compiled default relay set from the Android build.

Override values before packaging the beta:

- `beta.relaySetId` or `NDR_BETA_RELAY_SET_ID`
- `beta.relays` or `NDR_BETA_RELAYS`

Optional dedicated beta signing config:

- `beta.storeFile` or `NDR_BETA_KEYSTORE_PATH`
- `beta.storePassword` or `NDR_BETA_KEYSTORE_PASSWORD`
- `beta.keyAlias` or `NDR_BETA_KEY_ALIAS`
- `beta.keyPassword` or `NDR_BETA_KEY_PASSWORD`

If no dedicated beta signing config is supplied, the beta build falls back to the debug signing config.

## Support flow

Open the profile sheet to:

- share a redacted support bundle
- copy the support bundle JSON
- reset app state

The support bundle intentionally excludes:

- secret keys
- session secrets
- raw persisted protocol snapshots
- message bodies

## Automated gates

Canonical local entrypoints:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/test_fast.sh
./scripts/test_beta_local.sh
```

- `./scripts/test_fast.sh`
  - app Rust suite
  - one local-relay scenario soak iteration
- `./scripts/test_beta_local.sh`
  - library Rust suite
  - app Rust suite
  - local Android compile gates for debug/beta/androidTest

Recommended full beta gate before sending an APK to testers:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./scripts/test_beta_local.sh
./scripts/local_relay_scenario_soak.sh --iterations 100
./scripts/linked_device_relay_matrix.sh
./scripts/group_chat_restore_smoke.sh
./scripts/group_chat_matrix_smoke.sh
./gradlew :app:assembleBeta
```

The relay/device scripts remain explicit manual gates and are not part of the default fast path.

## Packaging

Build the beta APK:

```bash
cd /Users/l/Projects/iris-fork/ndr-demo-app
./gradlew :app:assembleBeta
```

Output:

- `app/build/outputs/apk/beta/app-beta.apk`

## Manual acceptance

Before inviting testers, manually verify on a real phone:

1. Install the beta APK.
2. Confirm the trusted-test warning is visible.
3. Create an account.
4. Link a second device.
5. Send and receive a direct message.
6. Create a group.
7. Send and receive group messages.
8. Remove a member and confirm future sends are blocked.
9. Force-stop and reopen the app, then verify chats/groups restore.
10. Export and share a support bundle.

## Upgrade smoke

Before publishing beta `N+1`:

1. Install beta `N`.
2. Create direct chats and a group.
3. Install beta `N+1` over beta `N`.
4. Reopen the app and verify restore.
5. Send and receive both direct and group messages again.
