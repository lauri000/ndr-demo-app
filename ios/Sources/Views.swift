import SwiftUI
import UIKit

struct RootView: View {
    @ObservedObject var manager: AppManager
    @State private var showingProfile = false

    var body: some View {
        ZStack(alignment: .top) {
            NavigationShell(
                title: screenTitle(manager.activeScreen),
                canGoBack: manager.canNavigateBack,
                onBack: manager.navigateBack,
                trailing: {
                    if case .chatList = manager.activeScreen, manager.state.account != nil {
                        Button("Profile") { showingProfile = true }
                            .accessibilityIdentifier("chatListProfileButton")
                    }
                }
            ) {
                content
            }

            if let toast = manager.toastMessage {
                Text(toast)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(.ultraThinMaterial, in: Capsule())
                    .padding(.top, 12)
            }
        }
        .sheet(isPresented: $showingProfile) {
            ProfileSheet(manager: manager)
        }
        .overlay {
            if manager.bootstrapInFlight {
                ProgressView("Loading")
                    .padding(24)
                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 20))
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        switch manager.activeScreen {
        case .welcome:
            WelcomeScreen(manager: manager)
        case .chatList:
            ChatListScreen(manager: manager)
        case .newChat:
            NewChatScreen(manager: manager)
        case .newGroup:
            NewGroupScreen(manager: manager)
        case .chat(let chatId):
            ChatScreen(manager: manager, chatId: chatId)
        case .groupDetails(let groupId):
            GroupDetailsScreen(manager: manager, groupId: groupId)
        case .deviceRoster:
            DeviceRosterScreen(manager: manager)
        case .awaitingDeviceApproval:
            AwaitingDeviceApprovalScreen(manager: manager)
        case .deviceRevoked:
            DeviceRevokedScreen(manager: manager)
        }
    }

    private func screenTitle(_ screen: Screen) -> String {
        switch screen {
        case .welcome: return "Welcome"
        case .chatList: return "Chats"
        case .newChat: return "New Chat"
        case .newGroup: return "New Group"
        case .chat: return manager.state.currentChat?.displayName ?? "Chat"
        case .groupDetails: return "Group"
        case .deviceRoster: return "Manage Devices"
        case .awaitingDeviceApproval: return "Approve Device"
        case .deviceRevoked: return "Device Revoked"
        }
    }
}

struct NavigationShell<Trailing: View, Content: View>: View {
    let title: String
    let canGoBack: Bool
    let onBack: () -> Void
    @ViewBuilder let trailing: () -> Trailing
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                if canGoBack {
                    Button(action: onBack) {
                        Label("Back", systemImage: "chevron.left")
                    }
                    .accessibilityIdentifier("navigationBackButton")
                } else {
                    Color.clear.frame(width: 44, height: 44)
                }
                Spacer()
                Text(title).font(.headline)
                Spacer()
                trailing().frame(minWidth: 44, alignment: .trailing)
            }
            .padding(.horizontal)
            .padding(.top, 12)
            .padding(.bottom, 8)
            Divider()
            content()
        }
    }
}

struct WelcomeScreen: View {
    @ObservedObject var manager: AppManager
    @State private var displayName = ""
    @State private var restoreInput = ""
    @State private var ownerInput = ""
    @State private var showingScanner = false

    private var normalizedOwnerInput: String {
        normalizePeerInput(input: ownerInput)
    }

    private var validOwnerInput: Bool {
        !normalizedOwnerInput.isEmpty && isValidPeerInput(input: normalizedOwnerInput)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                GroupBox("Create account") {
                    VStack(alignment: .leading, spacing: 12) {
                        TextField("Display name", text: $displayName)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("signupNameField")
                        Button(manager.state.busy.creatingAccount ? "Creating…" : "Generate new key") {
                            manager.createAccount(name: displayName)
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || manager.state.busy.creatingAccount)
                        .accessibilityIdentifier("generateKeyButton")
                    }
                }

                if manager.trustedTestBuildEnabled() {
                    GroupBox("Trusted test build") {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("This beta uses a controlled relay set and should not be used for sensitive conversations.")
                            Text(manager.buildSummaryText())
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                        }
                    }
                }

                GroupBox("Restore owner") {
                    VStack(alignment: .leading, spacing: 12) {
                        TextField("Owner nsec", text: $restoreInput, axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("importKeyField")
                        Button(manager.state.busy.restoringSession ? "Restoring…" : "Import existing key") {
                            manager.restoreSession(ownerNsec: restoreInput)
                        }
                        .buttonStyle(.bordered)
                        .disabled(restoreInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || manager.state.busy.restoringSession)
                        .accessibilityIdentifier("importKeyButton")
                    }
                }

                GroupBox("Link device") {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Scan the owner QR from the primary device. This device will publish its own invite and wait for explicit owner approval.")
                            .foregroundStyle(.secondary)
                        TextField("Owner npub or hex", text: $ownerInput, axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("linkOwnerInput")
                        if !ownerInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !validOwnerInput {
                            Text("Scanned or pasted owner key is not valid.")
                                .font(.footnote)
                                .foregroundStyle(.red)
                        }
                        HStack {
                            Button("Scan owner QR") { showingScanner = true }
                                .accessibilityIdentifier("linkOwnerScanQrButton")
                            Button(manager.state.busy.linkingDevice ? "Linking…" : "Link device") {
                                manager.startLinkedDevice(ownerInput: normalizedOwnerInput)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(!validOwnerInput || manager.state.busy.linkingDevice)
                            .accessibilityIdentifier("linkExistingAccountButton")
                        }
                    }
                }
            }
            .padding()
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                ownerInput = normalizePeerInput(input: code)
                showingScanner = false
            }
        }
    }
}

struct ChatListScreen: View {
    @ObservedObject var manager: AppManager

    var body: some View {
        List {
            Section("Actions") {
                Button("New chat") { manager.dispatch(.pushScreen(screen: .newChat)) }
                    .accessibilityIdentifier("chatListNewChatButton")
                Button("New group") { manager.dispatch(.pushScreen(screen: .newGroup)) }
                    .accessibilityIdentifier("chatListNewGroupButton")
            }

            if let account = manager.state.account {
                Section("Me") {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(account.displayName.isEmpty ? account.npub : account.displayName)
                            .font(.headline)
                        Text(account.npub)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Section("Chats") {
                ForEach(manager.state.chatList, id: \.chatId) { chat in
                    Button {
                        manager.dispatch(.openChat(chatId: chat.chatId))
                    } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(chat.displayName).font(.headline)
                                Spacer()
                                if chat.unreadCount > 0 {
                                    Text("\(chat.unreadCount)")
                                        .font(.caption.bold())
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 3)
                                        .background(.blue.opacity(0.15), in: Capsule())
                                }
                            }
                            if let preview = chat.lastMessagePreview {
                                Text(preview).lineLimit(2).foregroundStyle(.secondary)
                            }
                        }
                    }
                    .accessibilityIdentifier("chatRow-\(String(chat.chatId.prefix(12)))")
                }
            }
        }
        .listStyle(.insetGrouped)
    }
}

struct NewChatScreen: View {
    @ObservedObject var manager: AppManager
    @State private var peerInput = ""
    @State private var showingScanner = false

    private var normalizedPeerInput: String {
        normalizePeerInput(input: peerInput)
    }

    private var validPeerInput: Bool {
        !normalizedPeerInput.isEmpty && isValidPeerInput(input: normalizedPeerInput)
    }

    var body: some View {
        Form {
            Section("Peer key") {
                TextField("npub, hex, or nostr:...", text: $peerInput, axis: .vertical)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("newChatPeerInput")
                if !peerInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !validPeerInput {
                    Text("Not a valid nostr public key.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                HStack {
                    Button("Paste") {
                        peerInput = normalizePeerInput(input: UIPasteboard.general.string ?? "")
                    }
                    .accessibilityIdentifier("newChatPasteButton")
                    Button("Scan QR") { showingScanner = true }
                        .accessibilityIdentifier("newChatScanQrButton")
                }
                Button(manager.state.busy.creatingChat ? "Creating…" : "Open chat") {
                    manager.dispatch(.createChat(peerInput: normalizedPeerInput))
                }
                .buttonStyle(.borderedProminent)
                .disabled(!validPeerInput || manager.state.busy.creatingChat)
                .accessibilityIdentifier("newChatStartButton")
            }
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                peerInput = normalizePeerInput(input: code)
                showingScanner = false
            }
        }
    }
}

struct NewGroupScreen: View {
    @ObservedObject var manager: AppManager
    @State private var name = ""
    @State private var memberInput = ""
    @State private var selectedOwners = Set<String>()
    @State private var showingScanner = false

    private var normalizedMemberInput: String {
        normalizePeerInput(input: memberInput)
    }

    private var localOwnerHex: String? {
        manager.state.account?.publicKeyHex
    }

    private var existingDirectChats: [ChatThreadSnapshot] {
        manager.state.chatList.filter { chat in
            chat.kind == .direct && chat.chatId != localOwnerHex
        }
    }

    private var canCreate: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            !selectedOwners.isEmpty &&
            !manager.state.busy.creatingGroup
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                GroupBox("Group name") {
                    TextField("Weekend plans", text: $name)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("newGroupNameInput")
                }

                GroupBox("Add members") {
                    VStack(alignment: .leading, spacing: 12) {
                        TextField("npub, hex, or nostr:...", text: $memberInput, axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("newGroupMemberInput")
                        HStack {
                            Button("Paste") {
                                memberInput = normalizePeerInput(input: UIPasteboard.general.string ?? "")
                            }
                            .accessibilityIdentifier("newGroupPasteButton")
                            Button("Scan QR") { showingScanner = true }
                                .accessibilityIdentifier("newGroupScanQrButton")
                            Button("Add") {
                                addMember(normalizedMemberInput)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(!isValidPeerInput(input: normalizedMemberInput))
                            .accessibilityIdentifier("newGroupAddMemberButton")
                        }

                        if !selectedOwners.isEmpty {
                            VStack(alignment: .leading, spacing: 8) {
                                ForEach(selectedOwners.sorted(), id: \.self) { owner in
                                    HStack {
                                        Text(owner).font(.footnote.monospaced())
                                        Spacer()
                                        Button("Remove") {
                                            selectedOwners.remove(owner)
                                        }
                                        .foregroundStyle(.red)
                                        .accessibilityIdentifier("memberChipRemove")
                                    }
                                }
                            }
                        }
                    }
                }

                if !existingDirectChats.isEmpty {
                    GroupBox("Existing chats") {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(existingDirectChats, id: \.chatId) { chat in
                                let selected = selectedOwners.contains(chat.chatId)
                                Button(selected ? "Selected: \(chat.displayName)" : chat.displayName) {
                                    if selected {
                                        selectedOwners.remove(chat.chatId)
                                    } else {
                                        selectedOwners.insert(chat.chatId)
                                    }
                                }
                                .buttonStyle(.bordered)
                            }
                        }
                    }
                }

                Button(manager.state.busy.creatingGroup ? "Creating…" : "Create group") {
                    manager.dispatch(
                        .createGroup(
                            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
                            memberInputs: selectedOwners.sorted()
                        )
                    )
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canCreate)
                .accessibilityIdentifier("newGroupCreateButton")
            }
            .padding()
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                addMember(code)
                showingScanner = false
            }
        }
    }

    private func addMember(_ raw: String) {
        let normalized = normalizePeerInput(input: raw)
        guard !normalized.isEmpty, isValidPeerInput(input: normalized) else {
            return
        }
        guard normalized != localOwnerHex else {
            return
        }
        selectedOwners.insert(normalized)
        memberInput = ""
    }
}

struct ChatScreen: View {
    @ObservedObject var manager: AppManager
    let chatId: String
    @State private var draft = ""

    var body: some View {
        VStack(spacing: 0) {
            if let chat = manager.state.currentChat {
                List(chat.messages, id: \.id) { message in
                    VStack(alignment: message.isOutgoing ? .trailing : .leading, spacing: 4) {
                        Text(message.author)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(message.body)
                            .padding(10)
                            .background(
                                message.isOutgoing ? Color.blue.opacity(0.18) : Color.secondary.opacity(0.12),
                                in: RoundedRectangle(cornerRadius: 12)
                            )
                    }
                    .frame(maxWidth: .infinity, alignment: message.isOutgoing ? .trailing : .leading)
                    .listRowSeparator(.hidden)
                    .accessibilityIdentifier("chatMessage-\(message.id)")
                }
                .listStyle(.plain)
            } else {
                Spacer()
            }

            Divider()
            HStack(spacing: 12) {
                if manager.state.currentChat?.kind == .group,
                   let groupId = manager.state.currentChat?.groupId {
                    Button("Group") {
                        manager.dispatch(.pushScreen(screen: .groupDetails(groupId: groupId)))
                    }
                    .accessibilityIdentifier("chatGroupDetailsButton")
                }
                TextField("Message", text: $draft, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("chatMessageInput")
                Button(manager.state.busy.sendingMessage ? "Sending…" : "Send") {
                    let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                    draft = ""
                    manager.dispatch(.sendMessage(chatId: chatId, text: text))
                }
                .buttonStyle(.borderedProminent)
                .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || manager.state.busy.sendingMessage)
                .accessibilityIdentifier("chatSendButton")
            }
            .padding()
        }
    }
}

struct GroupDetailsScreen: View {
    @ObservedObject var manager: AppManager
    let groupId: String
    @State private var groupName = ""
    @State private var memberInput = ""
    @State private var showingScanner = false

    private var normalizedMemberInput: String {
        normalizePeerInput(input: memberInput)
    }

    var body: some View {
        Form {
            Color.clear
                .frame(height: 0)
                .accessibilityIdentifier("groupDetailsScreen")
            if let details = manager.state.groupDetails {
                Section("Group") {
                    TextField("Name", text: Binding(
                        get: { groupName.isEmpty ? details.name : groupName },
                        set: { groupName = $0 }
                    ))
                    .accessibilityIdentifier("groupDetailsNameInput")
                    if details.canManage {
                        Button(manager.state.busy.updatingGroup ? "Renaming…" : "Rename") {
                            let nextName = groupName.trimmingCharacters(in: .whitespacesAndNewlines)
                            manager.dispatch(.updateGroupName(groupId: groupId, name: nextName.isEmpty ? details.name : nextName))
                        }
                        .disabled(manager.state.busy.updatingGroup)
                        .accessibilityIdentifier("groupDetailsRenameButton")
                    }
                }

                Section("Members") {
                    ForEach(details.members, id: \.ownerPubkeyHex) { member in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(member.displayName)
                                Text(member.npub).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if details.canManage && !member.isLocalOwner {
                                Button("Remove", role: .destructive) {
                                    manager.dispatch(.removeGroupMember(groupId: groupId, ownerPubkeyHex: member.ownerPubkeyHex))
                                }
                                .accessibilityIdentifier("groupDetailsRemoveMember-\(String(member.ownerPubkeyHex.prefix(12)))")
                            }
                        }
                    }
                }

                if details.canManage {
                    Section("Add members") {
                        TextField("Member npub, hex, or nostr:...", text: $memberInput, axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .accessibilityIdentifier("groupDetailsAddMemberInput")
                        HStack {
                            Button("Scan member QR") { showingScanner = true }
                                .accessibilityIdentifier("groupDetailsScanQrButton")
                            Button(manager.state.busy.updatingGroup ? "Adding…" : "Add members") {
                                manager.dispatch(.addGroupMembers(groupId: groupId, memberInputs: [normalizedMemberInput]))
                                memberInput = ""
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(!isValidPeerInput(input: normalizedMemberInput) || manager.state.busy.updatingGroup)
                            .accessibilityIdentifier("groupDetailsAddMembersButton")
                        }
                    }
                }
            }
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                memberInput = normalizePeerInput(input: code)
                showingScanner = false
            }
        }
    }
}

struct DeviceRosterScreen: View {
    @ObservedObject var manager: AppManager
    @State private var deviceInput = ""
    @State private var showingScanner = false

    private var resolvedInput: ResolvedDeviceAuthorizationInput? {
        guard let roster = manager.state.deviceRoster else {
            return nil
        }
        return resolveDeviceAuthorizationInput(
            rawInput: deviceInput,
            ownerNpub: roster.ownerNpub,
            ownerPublicKeyHex: roster.ownerPublicKeyHex
        )
    }

    var body: some View {
        Form {
            if let roster = manager.state.deviceRoster {
                Section("Devices") {
                    Text(roster.ownerNpub)
                        .font(.footnote.monospaced())
                        .accessibilityIdentifier("deviceRosterOwnerNpub")
                    Text(roster.currentDeviceNpub)
                        .font(.footnote.monospaced())
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("deviceRosterCurrentDeviceNpub")
                }

                Section("Approve a new device") {
                    Text("New linked devices should appear here automatically after they scan your owner QR. You can still scan an approval QR or paste a device npub as fallback.")
                        .foregroundStyle(.secondary)
                    TextField("Device npub, hex, or approval code", text: $deviceInput, axis: .vertical)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("deviceRosterAddInput")
                    if let error = resolvedInput?.errorMessage {
                        Text(error).font(.footnote).foregroundStyle(.red)
                    }
                    HStack {
                        Button("Scan QR") { showingScanner = true }
                            .accessibilityIdentifier("deviceRosterScanButton")
                        Button(manager.state.busy.updatingRoster ? "Authorizing…" : "Authorize") {
                            let normalized = resolvedInput?.deviceInput ?? ""
                            manager.dispatch(.addAuthorizedDevice(deviceInput: normalized))
                            deviceInput = ""
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(
                            roster.canManageDevices == false ||
                            manager.state.busy.updatingRoster ||
                            (resolvedInput?.deviceInput.isEmpty ?? true)
                        )
                        .accessibilityIdentifier("deviceRosterAddButton")
                    }
                }

                Section("Device list") {
                    ForEach(roster.devices, id: \.devicePubkeyHex) { device in
                        DeviceRosterRow(manager: manager, device: device, canManageDevices: roster.canManageDevices)
                    }
                }
            } else {
                Text("No roster available.")
            }
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                deviceInput = code
                showingScanner = false
            }
        }
    }
}

private struct DeviceRosterRow: View {
    @ObservedObject var manager: AppManager
    let device: DeviceEntrySnapshot
    let canManageDevices: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(device.isCurrentDevice ? "\(device.deviceNpub) (this device)" : device.deviceNpub)
                .font(.footnote.monospaced())
            HStack {
                Text(device.isAuthorized ? "Authorized" : "Pending")
                    .foregroundStyle(device.isAuthorized ? .green : .orange)
                if device.isStale {
                    Text("Stale").foregroundStyle(.red)
                }
            }
            if canManageDevices && !device.isCurrentDevice {
                HStack {
                    if !device.isAuthorized {
                        Button(manager.state.busy.updatingRoster ? "Approving…" : "Approve") {
                            manager.dispatch(.addAuthorizedDevice(deviceInput: device.devicePubkeyHex))
                        }
                        .disabled(manager.state.busy.updatingRoster)
                        .accessibilityIdentifier("deviceRosterApprove-\(String(device.devicePubkeyHex.prefix(12)))")
                    }
                    Button("Remove device", role: .destructive) {
                        manager.dispatch(.removeAuthorizedDevice(devicePubkeyHex: device.devicePubkeyHex))
                    }
                    .disabled(manager.state.busy.updatingRoster)
                    .accessibilityIdentifier("deviceRosterRemove-\(String(device.devicePubkeyHex.prefix(12)))")
                }
            }
        }
        .accessibilityIdentifier("deviceRosterRow-\(String(device.devicePubkeyHex.prefix(12)))")
    }
}

struct AwaitingDeviceApprovalScreen: View {
    @ObservedObject var manager: AppManager

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("awaitingApprovalScreen")
                Text("Open the owner device and approve this device from Manage Devices.")
                    .multilineTextAlignment(.center)
                if let account = manager.state.account {
                    let qr = DeviceApprovalQr.encode(ownerInput: account.npub, deviceInput: account.deviceNpub)
                    ZStack {
                        QrCodeImage(text: qr)
                            .frame(width: 240, height: 240)
                        Color.clear
                            .accessibilityIdentifier("awaitingApprovalDeviceQrCode")
                    }
                    .frame(width: 240, height: 240)
                    Text(account.npub)
                        .font(.footnote.monospaced())
                        .textSelection(.enabled)
                        .accessibilityIdentifier("awaitingApprovalOwnerNpub")
                    Text(account.deviceNpub)
                        .font(.footnote.monospaced())
                        .textSelection(.enabled)
                        .accessibilityIdentifier("awaitingApprovalDeviceNpub")
                    Button("Copy device QR") {
                        manager.copyToClipboard(qr)
                    }
                    .accessibilityIdentifier("awaitingApprovalCopyDeviceButton")
                }
            }
            .padding()
        }
    }
}

struct DeviceRevokedScreen: View {
    @ObservedObject var manager: AppManager

    var body: some View {
        VStack(spacing: 16) {
            Text("This device has been removed from the roster.")
                .font(.headline)
                .multilineTextAlignment(.center)
            Button("Acknowledge") {
                manager.dispatch(.acknowledgeRevokedDevice)
            }
            .buttonStyle(.borderedProminent)
            .accessibilityIdentifier("deviceRevokedLogoutButton")
        }
        .padding()
        .accessibilityIdentifier("deviceRevokedScreen")
    }
}

struct ProfileSheet: View {
    @ObservedObject var manager: AppManager
    @Environment(\.dismiss) private var dismiss
    @State private var shareText: String?

    var body: some View {
        NavigationStack {
            List {
                if let account = manager.state.account {
                    Section("Owner") {
                        Button {
                            dismiss()
                            manager.dispatch(.pushScreen(screen: .deviceRoster))
                        } label: {
                            Text("Manage devices")
                                .accessibilityIdentifier("myProfileManageDevicesButton")
                        }
                        QrCodeImage(text: account.npub)
                            .frame(height: 220)
                            .accessibilityIdentifier("myProfileQrCode")
                        Text(account.npub)
                            .font(.footnote.monospaced())
                            .accessibilityIdentifier("myProfileNpubValue")
                        Text(account.deviceNpub)
                            .font(.footnote.monospaced())
                            .foregroundStyle(.secondary)
                        Button("Copy owner npub") { manager.copyToClipboard(account.npub) }
                        Button("Copy device npub") { manager.copyToClipboard(account.deviceNpub) }
                    }
                }

                if manager.trustedTestBuildEnabled() {
                    Section("Trusted test build") {
                        Text("This beta uses a controlled relay set and is intended for trusted testing only.")
                    }
                }

                Section("Support") {
                    Text("Build \(manager.buildSummaryText())")
                    Text("Relay set \(manager.relaySetIdText())")
                        .foregroundStyle(.secondary)
                    Button("Share support bundle") {
                        shareText = manager.supportBundleJson()
                    }
                    .accessibilityIdentifier("myProfileShareSupportBundleButton")
                    Button("Copy support bundle") {
                        manager.copyToClipboard(manager.supportBundleJson())
                    }
                    .accessibilityIdentifier("myProfileCopySupportBundleButton")
                    Button("Reset app state", role: .destructive) {
                        dismiss()
                        manager.resetAppState()
                    }
                    .accessibilityIdentifier("myProfileResetStateButton")
                }

                Section {
                    Button("Logout", role: .destructive) {
                        manager.logout()
                        dismiss()
                    }
                    .accessibilityIdentifier("myProfileLogoutButton")
                }
            }
            .navigationTitle("Profile")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .accessibilityIdentifier("myProfileSheet")
        .sheet(item: Binding(
            get: { shareText.map(SharePayload.init(text:)) },
            set: { shareText = $0?.text }
        )) { payload in
            ShareSheet(text: payload.text)
        }
    }
}

private struct SharePayload: Identifiable {
    let id = UUID()
    let text: String
}

private struct ShareSheet: UIViewControllerRepresentable {
    let text: String

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [text], applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
