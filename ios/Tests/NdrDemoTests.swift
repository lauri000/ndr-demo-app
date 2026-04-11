import XCTest
@testable import NdrDemo

private final class InMemorySecretStore: AccountSecretStore {
    var bundle: StoredAccountBundle?

    init(bundle: StoredAccountBundle? = nil) {
        self.bundle = bundle
    }

    func load() -> StoredAccountBundle? {
        bundle
    }

    func save(_ bundle: StoredAccountBundle) {
        self.bundle = bundle
    }

    func clear() {
        bundle = nil
    }
}

private final class MockRustApp: RustAppClient {
    var currentState: AppState
    var dispatchedActions: [AppAction] = []
    var supportBundleJson = "{\"ok\":true}"
    private var reconciler: AppReconciler?

    init(state: AppState = AppState(
        rev: 0,
        router: Router(defaultScreen: .welcome, screenStack: []),
        account: nil,
        deviceRoster: nil,
        busy: BusyState(
            creatingAccount: false,
            restoringSession: false,
            linkingDevice: false,
            creatingChat: false,
            creatingGroup: false,
            sendingMessage: false,
            updatingRoster: false,
            updatingGroup: false,
            syncingNetwork: false
        ),
        chatList: [],
        currentChat: nil,
        groupDetails: nil,
        toast: nil
    )) {
        self.currentState = state
    }

    func state() -> AppState {
        currentState
    }

    func dispatch(action: AppAction) {
        dispatchedActions.append(action)
    }

    func exportSupportBundleJson() -> String {
        supportBundleJson
    }

    func listenForUpdates(reconciler: AppReconciler) {
        self.reconciler = reconciler
    }

    func emit(_ update: AppUpdate) {
        reconciler?.reconcile(update: update)
    }
}

final class NdrDemoTests: XCTestCase {
    func testDeviceApprovalQrRoundTrip() {
        let encoded = DeviceApprovalQr.encode(ownerInput: "npub-owner", deviceInput: "npub-device")
        let decoded = DeviceApprovalQr.decode(encoded)
        XCTAssertEqual(decoded, DeviceApprovalQrPayload(ownerInput: "npub-owner", deviceInput: "npub-device"))
    }

    func testResolveDeviceAuthorizationInputRejectsDifferentOwner() {
        let ownerNpub = "npub18w35g6gn47qwmryulxzvfucmujvrqqljjpapyl8x0rqaljh6f2usml77dj"
        let otherOwner = "npub1m40q2j9vq7yrmgaf4q4f5a30gq2r6hwhzmu7t4j50c5f8ga2g8vs3hmzdt"
        let device = "npub1p34efzmkewwdsksmpp2r0tk7quke9jcfdz2zl7ezk8wnsj43uz2s8x5sp4"
        let qr = DeviceApprovalQr.encode(ownerInput: otherOwner, deviceInput: device)

        let resolved = resolveDeviceAuthorizationInput(
            rawInput: qr,
            ownerNpub: ownerNpub,
            ownerPublicKeyHex: normalizePeerInput(input: ownerNpub)
        )

        XCTAssertEqual(resolved.deviceInput, "")
        XCTAssertEqual(resolved.errorMessage, "This approval QR belongs to a different owner.")
    }

    func testKeychainSecretStoreRoundTrip() {
        let service = "social.innode.ndr.demo.tests.\(UUID().uuidString)"
        let account = "stored-account-bundle"
        let store = KeychainSecretStore(service: service, account: account)
        let expected = StoredAccountBundle(
            ownerNsec: "nsec1owner",
            ownerPubkeyHex: "owner-hex",
            deviceNsec: "nsec1device"
        )

        store.clear()
        store.save(expected)
        XCTAssertEqual(store.load(), expected)
        store.clear()
        XCTAssertNil(store.load())
    }

    @MainActor
    func testAppManagerRestoresPersistedBundleOnLaunch() async {
        let store = InMemorySecretStore(
            bundle: StoredAccountBundle(
                ownerNsec: "nsec1owner",
                ownerPubkeyHex: "owner-hex",
                deviceNsec: "nsec1device"
            )
        )
        let rust = MockRustApp()
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        _ = AppManager(
            rust: rust,
            secretStore: store,
            dataDir: tempDir,
            environment: [:]
        )

        await Task.yield()
        guard let first = rust.dispatchedActions.first else {
            return XCTFail("expected restore action")
        }
        switch first {
        case .restoreAccountBundle(let ownerNsec, let ownerPubkeyHex, let deviceNsec):
            XCTAssertEqual(ownerNsec, "nsec1owner")
            XCTAssertEqual(ownerPubkeyHex, "owner-hex")
            XCTAssertEqual(deviceNsec, "nsec1device")
        default:
            XCTFail("unexpected action \(first)")
        }
    }

    @MainActor
    func testAppManagerAppliesNewestFullStateOnly() async {
        let rust = MockRustApp()
        let store = InMemorySecretStore()
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let manager = AppManager(
            rust: rust,
            secretStore: store,
            dataDir: tempDir,
            environment: [:]
        )

        let newer = AppState(
            rev: 2,
            router: Router(defaultScreen: .chatList, screenStack: []),
            account: nil,
            deviceRoster: nil,
            busy: BusyState(
                creatingAccount: false,
                restoringSession: false,
                linkingDevice: false,
                creatingChat: false,
                creatingGroup: false,
                sendingMessage: false,
                updatingRoster: false,
                updatingGroup: false,
                syncingNetwork: false
            ),
            chatList: [],
            currentChat: nil,
            groupDetails: nil,
            toast: "synced"
        )
        let older = AppState(
            rev: 1,
            router: Router(defaultScreen: .welcome, screenStack: []),
            account: nil,
            deviceRoster: nil,
            busy: BusyState(
                creatingAccount: false,
                restoringSession: false,
                linkingDevice: false,
                creatingChat: false,
                creatingGroup: false,
                sendingMessage: false,
                updatingRoster: false,
                updatingGroup: false,
                syncingNetwork: false
            ),
            chatList: [],
            currentChat: nil,
            groupDetails: nil,
            toast: nil
        )

        rust.emit(.fullState(newer))
        await Task.yield()
        XCTAssertEqual(manager.state.rev, 2)
        XCTAssertEqual(manager.toastMessage, "synced")

        rust.emit(.fullState(older))
        await Task.yield()
        XCTAssertEqual(manager.state.rev, 2)
    }
}
