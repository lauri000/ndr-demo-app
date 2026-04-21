import SwiftUI
import UIKit

struct RootView: View {
    @ObservedObject var manager: AppManager
    @State private var showingProfile = false

    var body: some View {
        IrisTheme {
            ZStack(alignment: .top) {
                BackgroundFill()

                NavigationShell(
                    title: screenTitle(manager.activeScreen),
                    canGoBack: manager.canNavigateBack,
                    onBack: manager.navigateBack,
                    leading: topBarLeadingItem,
                    trailing: topBarTrailingItem
                ) {
                    content
                }

                if let toast = manager.toastMessage {
                    ToastView(text: toast)
                        .padding(.top, 14)
                }

                if manager.bootstrapInFlight {
                    LoadingOverlay()
                }
            }
            .sheet(isPresented: $showingProfile) {
                ProfileSheet(manager: manager)
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

    private var topBarLeadingItem: AnyView {
        guard case .chatList = manager.activeScreen, let account = manager.state.account else {
            return AnyView(EmptyView())
        }

        return AnyView(
            Button(action: { showingProfile = true }) {
                IrisAvatar(
                    label: account.displayName.isEmpty ? account.npub : account.displayName,
                    emphasize: true
                )
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("chatListProfileButton")
        )
    }

    private var topBarTrailingItem: AnyView {
        guard case .chat(let chatId) = manager.activeScreen,
              let chat = manager.state.currentChat,
              chat.chatId == chatId,
              chat.kind == .group,
              let groupId = chat.groupId else {
            return AnyView(EmptyView())
        }

        return AnyView(
            Button(action: {
                manager.dispatch(.pushScreen(screen: .groupDetails(groupId: groupId)))
            }) {
                Image(systemName: "person.3.fill")
                    .font(.system(size: 16, weight: .semibold))
                    .frame(width: 40, height: 40)
            }
            .buttonStyle(IrisSecondaryButtonStyle(compact: true))
            .accessibilityIdentifier("chatGroupDetailsButton")
        )
    }

    private func screenTitle(_ screen: Screen) -> String {
        switch screen {
        case .welcome: return "Welcome"
        case .chatList: return "Chats"
        case .newChat: return "New Chat"
        case .newGroup: return "New Group"
        case .chat:
            return manager.state.currentChat?.displayName ?? "Chat"
        case .groupDetails:
            return "Group"
        case .deviceRoster:
            return "Manage Devices"
        case .awaitingDeviceApproval:
            return "Approve Device"
        case .deviceRevoked:
            return "Device Revoked"
        }
    }
}

struct NavigationShell<Content: View>: View {
    let title: String
    let canGoBack: Bool
    let onBack: () -> Void
    let leading: AnyView
    let trailing: AnyView
    let content: () -> Content

    init(
        title: String,
        canGoBack: Bool,
        onBack: @escaping () -> Void,
        leading: AnyView = AnyView(EmptyView()),
        trailing: AnyView = AnyView(EmptyView()),
        @ViewBuilder content: @escaping () -> Content
    ) {
        self.title = title
        self.canGoBack = canGoBack
        self.onBack = onBack
        self.leading = leading
        self.trailing = trailing
        self.content = content
    }

    var body: some View {
        VStack(spacing: 0) {
            IrisTopBar(
                title: title,
                canGoBack: canGoBack,
                onBack: onBack,
                leading: leading,
                trailing: trailing
            )

            content()
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        }
    }
}

private struct OwnerPresentation {
    let primary: String
    let secondary: String?
}

private func trimmedText(_ value: String?) -> String? {
    guard let value else { return nil }
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
}

private func primaryDisplayName(displayName: String, fallback: String) -> String {
    trimmedText(displayName) ?? fallback
}

private func secondaryDisplayName(_ secondary: String?, primary: String) -> String? {
    guard let secondary = trimmedText(secondary) else {
        return nil
    }
    return secondary.caseInsensitiveCompare(primary) == .orderedSame ? nil : secondary
}

private func sameOwner(_ owner: String, hex: String?, npub: String?) -> Bool {
    let rawOwner = owner.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    let normalizedOwner = normalizePeerInput(input: owner).trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    let candidates = [hex, npub]
        .compactMap(trimmedText)
        .map { $0.lowercased() }
    return candidates.contains(rawOwner) || candidates.contains(normalizedOwner)
}

struct WelcomeScreen: View {
    @Environment(\.irisPalette) private var palette
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
        IrisScrollScreen {
            IrisSectionCard(accent: true) {
                Text("Iris Chat")
                    .font(.system(.largeTitle, design: .rounded, weight: .bold))
                    .foregroundStyle(palette.textPrimary)
                Text("Start with a fresh account, restore an owner, or link this device to an existing owner key.")
                    .font(.system(.body, design: .rounded))
                    .foregroundStyle(palette.muted)
            }

            IrisSectionCard {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("welcomeCreateCard")

                CardHeader(
                    title: "Create account",
                    subtitle: "Generate a new owner key and jump straight into chats."
                )

                TextField("Display name", text: $displayName)
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("signupNameField")

                Button(manager.state.busy.creatingAccount ? "Creating…" : "Generate new key") {
                    manager.createAccount(name: displayName)
                }
                .buttonStyle(IrisPrimaryButtonStyle())
                .disabled(
                    displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
                    manager.state.busy.creatingAccount
                )
                .accessibilityIdentifier("generateKeyButton")
            }

            if manager.trustedTestBuildEnabled() {
                IrisSectionCard(accent: true) {
                    CardHeader(
                        title: "Trusted test build",
                        subtitle: "This beta uses a controlled relay set and is not meant for sensitive conversations."
                    )

                    Text(manager.buildSummaryText())
                        .font(.system(.footnote, design: .monospaced))
                        .foregroundStyle(palette.muted)
                }
            }

            IrisSectionCard {
                CardHeader(
                    title: "Restore owner",
                    subtitle: "Bring an existing owner key onto this device."
                )

                TextField("Owner nsec", text: $restoreInput)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("importKeyField")

                Button(manager.state.busy.restoringSession ? "Restoring…" : "Import existing key") {
                    manager.restoreSession(ownerNsec: restoreInput)
                }
                .buttonStyle(IrisSecondaryButtonStyle())
                .disabled(
                    restoreInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
                    manager.state.busy.restoringSession
                )
                .accessibilityIdentifier("importKeyButton")
            }

            IrisSectionCard {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("welcomeLinkCard")

                CardHeader(
                    title: "Link device",
                    subtitle: "Scan the owner QR from the primary device, then wait for approval."
                )

                TextField("Owner npub or hex", text: $ownerInput)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("linkOwnerInput")

                if !ownerInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !validOwnerInput {
                    Text("Scanned or pasted owner key is not valid.")
                        .font(.system(.footnote, design: .rounded))
                        .foregroundStyle(.red)
                }

                VStack(spacing: 10) {
                    scanOwnerButton
                    linkOwnerButton
                }
            }
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                ownerInput = normalizePeerInput(input: code)
                showingScanner = false
            }
        }
    }

    private var scanOwnerButton: some View {
        Button("Scan owner QR") { showingScanner = true }
            .buttonStyle(IrisSecondaryButtonStyle())
            .accessibilityIdentifier("linkOwnerScanQrButton")
    }

    private var linkOwnerButton: some View {
        Button(manager.state.busy.linkingDevice ? "Linking…" : "Link device") {
            manager.startLinkedDevice(ownerInput: normalizedOwnerInput)
        }
        .buttonStyle(IrisPrimaryButtonStyle())
        .disabled(!validOwnerInput || manager.state.busy.linkingDevice)
        .accessibilityIdentifier("linkExistingAccountButton")
    }
}

struct ChatListScreen: View {
    @Environment(\.irisPalette) private var palette
    @ObservedObject var manager: AppManager

    var body: some View {
        IrisScrollScreen {
            IrisSectionCard(accent: true) {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("chatListHeroCard")

                CardHeader(
                    title: "Conversations",
                    subtitle: "Direct chats and groups live together here. Start something new or jump back into an active thread."
                )

                HStack(spacing: 10) {
                    newChatButton
                    newGroupButton
                }
            }

            if let account = manager.state.account {
                IrisSectionCard {
                    HStack(spacing: 14) {
                        IrisAvatar(label: account.displayName.isEmpty ? account.npub : account.displayName, emphasize: true)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(account.displayName.isEmpty ? "Your account" : account.displayName)
                                .font(.system(.headline, design: .rounded, weight: .semibold))
                                .foregroundStyle(palette.textPrimary)
                            Text(account.npub)
                                .font(.system(.footnote, design: .monospaced))
                                .foregroundStyle(palette.muted)
                                .lineLimit(2)
                        }
                    }
                }
            }

            if manager.state.chatList.isEmpty {
                IrisSectionCard {
                    Text("No chats yet")
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(palette.textPrimary)
                    Text("Create a direct chat with an npub or start a group with people you already know.")
                        .font(.system(.body, design: .rounded))
                        .foregroundStyle(palette.muted)
                }
            } else {
                IrisSectionCard {
                    ForEach(Array(manager.state.chatList.enumerated()), id: \.element.chatId) { index, chat in
                        IrisChatRow(
                            title: chat.displayName,
                            preview: chat.lastMessagePreview ?? chat.subtitle ?? "No messages yet",
                            subtitle: chat.kind == .group ? chat.subtitle : nil,
                            timeLabel: irisRelativeTime(chat.lastMessageAtSecs),
                            unreadCount: chat.unreadCount,
                            onTap: {
                                manager.dispatch(.openChat(chatId: chat.chatId))
                            }
                        )
                        .accessibilityIdentifier("chatRow-\(String(chat.chatId.prefix(12)))")

                        if index < manager.state.chatList.count - 1 {
                            Divider()
                                .overlay(palette.border)
                        }
                    }
                }
            }
        }
    }

    private var newChatButton: some View {
        Button {
            manager.dispatch(.pushScreen(screen: .newChat))
        } label: {
            Label("New chat", systemImage: "message.fill")
        }
        .buttonStyle(IrisPrimaryButtonStyle())
        .accessibilityIdentifier("chatListNewChatButton")
    }

    private var newGroupButton: some View {
        Button {
            manager.dispatch(.pushScreen(screen: .newGroup))
        } label: {
            Label("New group", systemImage: "person.3.fill")
        }
        .buttonStyle(IrisSecondaryButtonStyle())
        .accessibilityIdentifier("chatListNewGroupButton")
    }
}

struct NewChatScreen: View {
    @Environment(\.irisPalette) private var palette
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
        IrisScrollScreen {
            IrisSectionCard(accent: true) {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("newChatPrimaryCard")

                CardHeader(
                    title: "Direct chat",
                    subtitle: "Paste an npub, a hex key, or scan a QR code to open a one-to-one conversation."
                )

                TextField("npub, hex, or nostr:…", text: $peerInput)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("newChatPeerInput")

                if !peerInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !validPeerInput {
                    Text("Not a valid nostr public key.")
                        .font(.system(.footnote, design: .rounded))
                        .foregroundStyle(.red)
                }

                HStack(spacing: 10) {
                    pasteButton
                    scanButton
                }

                Button(manager.state.busy.creatingChat ? "Creating…" : "Open chat") {
                    manager.dispatch(.createChat(peerInput: normalizedPeerInput))
                }
                .buttonStyle(IrisPrimaryButtonStyle())
                .disabled(!validPeerInput || manager.state.busy.creatingChat)
                .accessibilityIdentifier("newChatStartButton")
            }

            IrisSectionCard {
                Text("Tip")
                    .font(.system(.headline, design: .rounded, weight: .semibold))
                    .foregroundStyle(palette.textPrimary)
                Text("You can paste `nostr:` links directly. The shell normalizes them before dispatching to Rust.")
                    .font(.system(.body, design: .rounded))
                    .foregroundStyle(palette.muted)
            }
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                peerInput = normalizePeerInput(input: code)
                showingScanner = false
            }
        }
    }

    private var pasteButton: some View {
        Button("Paste") {
            peerInput = normalizePeerInput(input: UIPasteboard.general.string ?? "")
        }
        .buttonStyle(IrisSecondaryButtonStyle())
        .accessibilityIdentifier("newChatPasteButton")
    }

    private var scanButton: some View {
        Button("Scan QR") { showingScanner = true }
            .buttonStyle(IrisSecondaryButtonStyle())
            .accessibilityIdentifier("newChatScanQrButton")
    }
}

struct NewGroupScreen: View {
    @Environment(\.irisPalette) private var palette
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

    private func ownerPresentation(for owner: String) -> OwnerPresentation {
        if let chat = existingDirectChats.first(where: { sameOwner(owner, hex: $0.chatId, npub: $0.subtitle) }) {
            let primary = primaryDisplayName(displayName: chat.displayName, fallback: normalizePeerInput(input: owner))
            return OwnerPresentation(
                primary: primary,
                secondary: secondaryDisplayName(chat.subtitle, primary: primary)
            )
        }

        if let account = manager.state.account, sameOwner(owner, hex: account.publicKeyHex, npub: account.npub) {
            let primary = primaryDisplayName(displayName: account.displayName, fallback: account.npub)
            return OwnerPresentation(
                primary: primary,
                secondary: secondaryDisplayName(account.npub, primary: primary)
            )
        }

        let normalized = normalizePeerInput(input: owner)
        return OwnerPresentation(primary: normalized, secondary: nil)
    }

    var body: some View {
        IrisScrollScreen {
            IrisSectionCard(accent: true) {
                Color.clear
                    .frame(height: 0)
                    .accessibilityIdentifier("newGroupPrimaryCard")

                CardHeader(
                    title: "Create group",
                    subtitle: "Choose a name, add people you already know, then manage the group from the thread."
                )

                TextField("Weekend plans", text: $name)
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("newGroupNameInput")
            }

            IrisSectionCard {
                CardHeader(
                    title: "Add members",
                    subtitle: "Paste or scan people directly, or pick from existing direct chats."
                )

                TextField("npub, hex, or nostr:…", text: $memberInput)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("newGroupMemberInput")

                VStack(spacing: 10) {
                    pasteMemberButton
                    scanMemberButton
                    addMemberButton
                }

                if !selectedOwners.isEmpty {
                    FlowWrap(spacing: 8, lineSpacing: 8) {
                        ForEach(selectedOwners.sorted(), id: \.self) { owner in
                            let presentation = ownerPresentation(for: owner)
                            SelectedMemberChip(
                                title: presentation.primary,
                                subtitle: presentation.secondary,
                                onRemove: { selectedOwners.remove(owner) }
                            )
                        }
                    }
                }
            }

            if !existingDirectChats.isEmpty {
                IrisSectionCard {
                    CardHeader(
                        title: "Existing chats",
                        subtitle: "Quick-pick people you already have in your chat list."
                    )

                    ForEach(Array(existingDirectChats.enumerated()), id: \.element.chatId) { index, chat in
                        Button {
                            if selectedOwners.contains(chat.chatId) {
                                selectedOwners.remove(chat.chatId)
                            } else {
                                selectedOwners.insert(chat.chatId)
                            }
                        } label: {
                            HStack(spacing: 12) {
                                IrisAvatar(label: chat.displayName, size: 38, emphasize: selectedOwners.contains(chat.chatId))
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(chat.displayName)
                                        .font(.system(.headline, design: .rounded, weight: .semibold))
                                        .foregroundStyle(palette.textPrimary)
                                    if let subtitle = secondaryDisplayName(chat.subtitle, primary: chat.displayName) {
                                        Text(subtitle)
                                            .font(.system(.footnote, design: .rounded))
                                            .foregroundStyle(palette.muted)
                                    }
                                }
                                Spacer()
                                Image(systemName: selectedOwners.contains(chat.chatId) ? "checkmark.circle.fill" : "circle")
                                    .foregroundStyle(selectedOwners.contains(chat.chatId) ? palette.accent : palette.muted)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)

                        if index < existingDirectChats.count - 1 {
                            Divider().overlay(palette.border)
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
            .buttonStyle(IrisPrimaryButtonStyle())
            .disabled(!canCreate)
            .accessibilityIdentifier("newGroupCreateButton")
        }
        .sheet(isPresented: $showingScanner) {
            QrScannerSheet { code in
                addMember(code)
                showingScanner = false
            }
        }
    }

    private var pasteMemberButton: some View {
        Button("Paste") {
            memberInput = normalizePeerInput(input: UIPasteboard.general.string ?? "")
        }
        .buttonStyle(IrisSecondaryButtonStyle())
        .accessibilityIdentifier("newGroupPasteButton")
    }

    private var scanMemberButton: some View {
        Button("Scan QR") { showingScanner = true }
            .buttonStyle(IrisSecondaryButtonStyle())
            .accessibilityIdentifier("newGroupScanQrButton")
    }

    private var addMemberButton: some View {
        Button("Add") {
            addMember(normalizedMemberInput)
        }
        .buttonStyle(IrisPrimaryButtonStyle())
        .disabled(!isValidPeerInput(input: normalizedMemberInput))
        .accessibilityIdentifier("newGroupAddMemberButton")
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
    @Environment(\.irisPalette) private var palette
    @ObservedObject var manager: AppManager
    let chatId: String

    @State private var draft = ""
    @State private var isNearBottom = true
    @State private var timelineViewportMaxY: CGFloat = 0
    @State private var timelineBottomMaxY: CGFloat = .greatestFiniteMagnitude
    @State private var initialScrollPending = true

    private var chat: CurrentChatSnapshot? {
        manager.state.currentChat?.chatId == chatId ? manager.state.currentChat : nil
    }

    var body: some View {
        VStack(spacing: 0) {
            if let chat {
                ScrollViewReader { proxy in
                    ZStack(alignment: .bottomTrailing) {
                        ScrollView {
                            LazyVStack(spacing: 0) {
                                ForEach(Array(chat.messages.enumerated()), id: \.element.id) { index, message in
                                    let previous = index > 0 ? chat.messages[index - 1] : nil
                                    let next = index + 1 < chat.messages.count ? chat.messages[index + 1] : nil
                                    let showDayChip = previous == nil || !irisSameTimelineDay(previous!.createdAtSecs, message.createdAtSecs)
                                    let isFirstInCluster = previous == nil || previous!.isOutgoing != message.isOutgoing
                                    let isLastInCluster = next == nil || next!.isOutgoing != message.isOutgoing

                                    ChatMessageRow(
                                        message: message,
                                        chatKind: chat.kind,
                                        showDayChip: showDayChip,
                                        isFirstInCluster: isFirstInCluster,
                                        isLastInCluster: isLastInCluster
                                    )
                                    .id(message.id)
                                }

                                Color.clear
                                    .frame(height: 1)
                                    .id(ChatTimelineAnchor.bottom)
                                    .background(
                                        GeometryReader { geometry in
                                            Color.clear.preference(
                                                key: ChatTimelineBottomMaxYPreferenceKey.self,
                                                value: geometry.frame(in: .named(ChatTimelineCoordinateSpace.name)).maxY
                                            )
                                        }
                                    )
                                    .accessibilityHidden(true)
                            }
                            .padding(.horizontal, 16)
                            .padding(.vertical, 12)
                            .accessibilityIdentifier("chatTimeline")
                        }
                        .coordinateSpace(name: ChatTimelineCoordinateSpace.name)
                        .overlay {
                            GeometryReader { geometry in
                                Color.clear.preference(
                                    key: ChatTimelineViewportMaxYPreferenceKey.self,
                                    value: geometry.frame(in: .named(ChatTimelineCoordinateSpace.name)).maxY
                                )
                            }
                        }
                        .scrollDismissesKeyboard(.interactively)
                        .onChange(of: chatId) { _ in
                            initialScrollPending = true
                            isNearBottom = true
                        }
                        .onPreferenceChange(ChatTimelineViewportMaxYPreferenceKey.self) { value in
                            timelineViewportMaxY = value
                            isNearBottom = chatTimelineIsNearBottom(
                                viewportMaxY: value,
                                bottomMaxY: timelineBottomMaxY
                            )
                        }
                        .onPreferenceChange(ChatTimelineBottomMaxYPreferenceKey.self) { value in
                            timelineBottomMaxY = value
                            isNearBottom = chatTimelineIsNearBottom(
                                viewportMaxY: timelineViewportMaxY,
                                bottomMaxY: value
                            )
                        }
                        .task(id: chat.messages.last?.id) {
                            guard !chat.messages.isEmpty else {
                                initialScrollPending = true
                                return
                            }
                            guard initialScrollPending || isNearBottom else {
                                return
                            }
                            scrollToBottom(proxy: proxy, animated: !initialScrollPending)
                            initialScrollPending = false
                        }

                        if !isNearBottom && !chat.messages.isEmpty {
                            Button {
                                scrollToBottom(proxy: proxy, animated: true)
                            } label: {
                                Image(systemName: "arrow.down")
                                    .font(.system(size: 18, weight: .bold))
                                    .foregroundStyle(palette.onAccent)
                                    .frame(width: 48, height: 48)
                                    .background(
                                        Circle()
                                            .fill(palette.accent)
                                            .overlay(
                                                Circle()
                                                    .stroke(palette.border.opacity(0.25), lineWidth: 1)
                                            )
                                    )
                            }
                            .padding(.trailing, 18)
                            .padding(.bottom, 18)
                            .buttonStyle(.plain)
                            .shadow(color: .black.opacity(0.16), radius: 16, y: 10)
                            .accessibilityIdentifier("chatJumpToBottom")
                        }
                    }
                }
            } else {
                Spacer()
                IrisSectionCard {
                    Text("Loading chat…")
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(palette.textPrimary)
                }
                .padding(.horizontal, 16)
                Spacer()
            }

            IrisComposerBar(
                draft: $draft,
                placeholder: "Message",
                isSending: manager.state.busy.sendingMessage
            ) {
                let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !text.isEmpty else { return }
                draft = ""
                manager.dispatch(.sendMessage(chatId: chatId, text: text))
            }
        }
    }

    private func scrollToBottom(proxy: ScrollViewProxy, animated: Bool) {
        DispatchQueue.main.async {
            if animated {
                withAnimation(.easeOut(duration: 0.2)) {
                    proxy.scrollTo(ChatTimelineAnchor.bottom, anchor: .bottom)
                }
            } else {
                proxy.scrollTo(ChatTimelineAnchor.bottom, anchor: .bottom)
            }
        }
    }
}

private enum ChatTimelineCoordinateSpace {
    static let name = "chatTimelineCoordinateSpace"
}

private enum ChatTimelineAnchor {
    static let bottom = "chatTimelineBottom"
}

private struct ChatTimelineViewportMaxYPreferenceKey: PreferenceKey {
    static var defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private struct ChatTimelineBottomMaxYPreferenceKey: PreferenceKey {
    static var defaultValue: CGFloat = .greatestFiniteMagnitude

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private func chatTimelineIsNearBottom(viewportMaxY: CGFloat, bottomMaxY: CGFloat) -> Bool {
    guard viewportMaxY > 0, bottomMaxY.isFinite else {
        return true
    }
    return bottomMaxY <= viewportMaxY + 24
}

struct GroupDetailsScreen: View {
    @Environment(\.irisPalette) private var palette
    @ObservedObject var manager: AppManager
    let groupId: String

    @State private var groupName = ""
    @State private var memberInput = ""
    @State private var showingScanner = false

    private var normalizedMemberInput: String {
        normalizePeerInput(input: memberInput)
    }

    var body: some View {
        IrisScrollScreen {
            Color.clear
                .frame(height: 0)
                .accessibilityIdentifier("groupDetailsScreen")

            if let details = manager.state.groupDetails {
                IrisSectionCard(accent: true) {
                    CardHeader(
                        title: "Group settings",
                        subtitle: "Created by \(details.createdByDisplayName). Revision \(details.revision)."
                    )

                    TextField("Name", text: Binding(
                        get: { groupName.isEmpty ? details.name : groupName },
                        set: { groupName = $0 }
                    ))
                    .textFieldStyle(.plain)
                    .irisInputField()
                    .accessibilityIdentifier("groupDetailsNameInput")

                    if details.canManage {
                        Button(manager.state.busy.updatingGroup ? "Renaming…" : "Rename") {
                            let nextName = groupName.trimmingCharacters(in: .whitespacesAndNewlines)
                            manager.dispatch(.updateGroupName(groupId: groupId, name: nextName.isEmpty ? details.name : nextName))
                        }
                        .buttonStyle(IrisPrimaryButtonStyle())
                        .disabled(manager.state.busy.updatingGroup)
                        .accessibilityIdentifier("groupDetailsRenameButton")
                    }
                }

                IrisSectionCard {
                    CardHeader(
                        title: "Members",
                        subtitle: "\(details.members.count) people in this conversation."
                    )

                    ForEach(Array(details.members.enumerated()), id: \.element.ownerPubkeyHex) { index, member in
                        let primary = primaryDisplayName(displayName: member.displayName, fallback: member.npub)
                        HStack(alignment: .top, spacing: 12) {
                            IrisAvatar(label: primary, size: 38, emphasize: member.isLocalOwner)

                            VStack(alignment: .leading, spacing: 4) {
                                Text(primary)
                                    .font(.system(.headline, design: .rounded, weight: .semibold))
                                    .foregroundStyle(palette.textPrimary)
                                if let secondary = secondaryDisplayName(member.npub, primary: primary) {
                                    Text(secondary)
                                        .font(.system(.footnote, design: .monospaced))
                                        .foregroundStyle(palette.muted)
                                        .lineLimit(2)
                                }
                                if member.isLocalOwner {
                                    IrisInfoPill("You")
                                }
                            }

                            Spacer()

                            if details.canManage && !member.isLocalOwner {
                                Button("Remove", role: .destructive) {
                                    manager.dispatch(.removeGroupMember(groupId: groupId, ownerPubkeyHex: member.ownerPubkeyHex))
                                }
                                .buttonStyle(IrisSecondaryButtonStyle(compact: true))
                                .accessibilityIdentifier("groupDetailsRemoveMember-\(String(member.ownerPubkeyHex.prefix(12)))")
                            }
                        }

                        if index < details.members.count - 1 {
                            Divider().overlay(palette.border)
                        }
                    }
                }

                if details.canManage {
                    IrisSectionCard {
                        CardHeader(
                            title: "Add members",
                            subtitle: "Approve a new member by scan or paste."
                        )

                        TextField("Member npub, hex, or nostr:…", text: $memberInput)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .textFieldStyle(.plain)
                            .irisInputField()
                            .accessibilityIdentifier("groupDetailsAddMemberInput")

                        VStack(spacing: 10) {
                            Button("Scan member QR") { showingScanner = true }
                                .buttonStyle(IrisSecondaryButtonStyle())
                                .accessibilityIdentifier("groupDetailsScanQrButton")

                            Button(manager.state.busy.updatingGroup ? "Adding…" : "Add members") {
                                manager.dispatch(.addGroupMembers(groupId: groupId, memberInputs: [normalizedMemberInput]))
                                memberInput = ""
                            }
                            .buttonStyle(IrisPrimaryButtonStyle())
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
    @Environment(\.irisPalette) private var palette
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
        IrisScrollScreen {
            if let roster = manager.state.deviceRoster {
                IrisSectionCard(accent: true) {
                    CardHeader(
                        title: "Owner devices",
                        subtitle: roster.canManageDevices ? "This device can approve and remove linked devices." : "This device can view linked devices only."
                    )

                    MonoValue(label: "Owner", value: roster.ownerNpub, identifier: "deviceRosterOwnerNpub")
                    MonoValue(label: "This device", value: roster.currentDeviceNpub, identifier: "deviceRosterCurrentDeviceNpub")
                }

                IrisSectionCard {
                    CardHeader(
                        title: "Approve a new device",
                        subtitle: "New linked devices should appear here automatically after they scan the owner QR."
                    )

                    TextField("Device npub, hex, or approval code", text: $deviceInput)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textFieldStyle(.plain)
                        .irisInputField()
                        .accessibilityIdentifier("deviceRosterAddInput")

                    if let error = resolvedInput?.errorMessage {
                        Text(error)
                            .font(.system(.footnote, design: .rounded))
                            .foregroundStyle(.red)
                    }

                    VStack(spacing: 10) {
                        Button("Scan QR") { showingScanner = true }
                            .buttonStyle(IrisSecondaryButtonStyle())
                            .accessibilityIdentifier("deviceRosterScanButton")
                        Button(manager.state.busy.updatingRoster ? "Authorizing…" : "Authorize") {
                            let normalized = resolvedInput?.deviceInput ?? ""
                            manager.dispatch(.addAuthorizedDevice(deviceInput: normalized))
                            deviceInput = ""
                        }
                        .buttonStyle(IrisPrimaryButtonStyle())
                        .disabled(
                            roster.canManageDevices == false ||
                            manager.state.busy.updatingRoster ||
                            (resolvedInput?.deviceInput.isEmpty ?? true)
                        )
                        .accessibilityIdentifier("deviceRosterAddButton")
                    }
                }

                IrisSectionCard {
                    CardHeader(
                        title: "Device list",
                        subtitle: "\(roster.devices.count) linked device(s)."
                    )

                    ForEach(Array(roster.devices.enumerated()), id: \.element.devicePubkeyHex) { index, device in
                        DeviceRosterRow(manager: manager, device: device, canManageDevices: roster.canManageDevices)
                        if index < roster.devices.count - 1 {
                            Divider().overlay(palette.border)
                        }
                    }
                }
            } else {
                IrisSectionCard {
                    Text("No roster available.")
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(palette.textPrimary)
                }
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
    @Environment(\.irisPalette) private var palette
    @ObservedObject var manager: AppManager
    let device: DeviceEntrySnapshot
    let canManageDevices: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                IrisAvatar(label: device.deviceNpub, size: 36, emphasize: device.isCurrentDevice)
                VStack(alignment: .leading, spacing: 4) {
                    Text(device.isCurrentDevice ? "This device" : "Linked device")
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(palette.textPrimary)
                    Text(device.deviceNpub)
                        .font(.system(.footnote, design: .monospaced))
                        .foregroundStyle(palette.muted)
                        .lineLimit(2)
                }
            }

            HStack(spacing: 8) {
                IrisInfoPill(device.isAuthorized ? "Authorized" : "Pending", tint: device.isAuthorized ? .green : .orange)
                if device.isStale {
                    IrisInfoPill("Stale", tint: .red)
                }
            }

            if canManageDevices && !device.isCurrentDevice {
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 10) {
                        if !device.isAuthorized {
                            approveButton
                        }
                        removeButton
                    }
                    VStack(spacing: 10) {
                        if !device.isAuthorized {
                            approveButton
                        }
                        removeButton
                    }
                }
            }
        }
        .accessibilityIdentifier("deviceRosterRow-\(String(device.devicePubkeyHex.prefix(12)))")
    }

    private var approveButton: some View {
        Button(manager.state.busy.updatingRoster ? "Approving…" : "Approve") {
            manager.dispatch(.addAuthorizedDevice(deviceInput: device.devicePubkeyHex))
        }
        .buttonStyle(IrisPrimaryButtonStyle())
        .disabled(manager.state.busy.updatingRoster)
        .accessibilityIdentifier("deviceRosterApprove-\(String(device.devicePubkeyHex.prefix(12)))")
    }

    private var removeButton: some View {
        Button("Remove device", role: .destructive) {
            manager.dispatch(.removeAuthorizedDevice(devicePubkeyHex: device.devicePubkeyHex))
        }
        .buttonStyle(IrisSecondaryButtonStyle())
        .disabled(manager.state.busy.updatingRoster)
        .accessibilityIdentifier("deviceRosterRemove-\(String(device.devicePubkeyHex.prefix(12)))")
    }
}

struct AwaitingDeviceApprovalScreen: View {
    @ObservedObject var manager: AppManager

    var body: some View {
        IrisScrollScreen {
            Color.clear
                .frame(height: 0)
                .accessibilityIdentifier("awaitingApprovalScreen")

            IrisSectionCard(accent: true) {
                CardHeader(
                    title: "Finish linking",
                    subtitle: "Open the owner device and approve this device from Manage Devices."
                )

                if let account = manager.state.account {
                    let qr = DeviceApprovalQr.encode(ownerInput: account.npub, deviceInput: account.deviceNpub)
                    ZStack {
                        QrCodeImage(text: qr)
                            .frame(width: 240, height: 240)
                        Color.clear
                            .accessibilityIdentifier("awaitingApprovalDeviceQrCode")
                    }
                    .frame(maxWidth: .infinity)

                    MonoValue(label: "Owner", value: account.npub, identifier: "awaitingApprovalOwnerNpub")
                    MonoValue(label: "This device", value: account.deviceNpub, identifier: "awaitingApprovalDeviceNpub")

                    Button("Copy device QR") {
                        manager.copyToClipboard(qr)
                    }
                    .buttonStyle(IrisPrimaryButtonStyle())
                    .accessibilityIdentifier("awaitingApprovalCopyDeviceButton")
                }
            }
        }
    }
}

struct DeviceRevokedScreen: View {
    @ObservedObject var manager: AppManager

    var body: some View {
        IrisScrollScreen {
            IrisSectionCard(accent: true) {
                Text("This device has been removed from the roster.")
                    .font(.system(.title3, design: .rounded, weight: .bold))
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)

                Text("Acknowledge this state to return to a fresh shell.")
                    .font(.system(.body, design: .rounded))
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)

                Button("Acknowledge") {
                    manager.dispatch(.acknowledgeRevokedDevice)
                }
                .buttonStyle(IrisPrimaryButtonStyle())
                .accessibilityIdentifier("deviceRevokedLogoutButton")
            }
            .accessibilityIdentifier("deviceRevokedScreen")
        }
    }
}

struct ProfileSheet: View {
    @Environment(\.irisPalette) private var palette
    @ObservedObject var manager: AppManager
    @Environment(\.dismiss) private var dismiss
    @State private var shareText: String?

    var body: some View {
        NavigationStack {
            ZStack {
                BackgroundFill()

                IrisScrollScreen {
                    if let account = manager.state.account {
                        IrisSectionCard(accent: true) {
                            HStack(spacing: 14) {
                                IrisAvatar(label: account.displayName.isEmpty ? account.npub : account.displayName, size: 52, emphasize: true)
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(account.displayName.isEmpty ? "Owner profile" : account.displayName)
                                        .font(.system(.title3, design: .rounded, weight: .bold))
                                        .foregroundStyle(palette.textPrimary)
                                    Text(account.npub)
                                        .font(.system(.footnote, design: .monospaced))
                                        .foregroundStyle(palette.muted)
                                        .lineLimit(2)
                                        .accessibilityIdentifier("myProfileNpubValue")
                                }
                            }

                            Button {
                                dismiss()
                                manager.dispatch(.pushScreen(screen: .deviceRoster))
                            } label: {
                                Label("Manage devices", systemImage: "laptopcomputer.and.iphone")
                            }
                            .buttonStyle(IrisSecondaryButtonStyle())
                            .accessibilityIdentifier("myProfileManageDevicesButton")

                            QrCodeImage(text: account.npub)
                                .frame(height: 220)
                                .frame(maxWidth: .infinity)
                                .accessibilityIdentifier("myProfileQrCode")

                            MonoValue(label: "Device", value: account.deviceNpub)

                            VStack(spacing: 10) {
                                Button("Copy owner npub") { manager.copyToClipboard(account.npub) }
                                    .buttonStyle(IrisSecondaryButtonStyle())
                                Button("Copy device npub") { manager.copyToClipboard(account.deviceNpub) }
                                    .buttonStyle(IrisSecondaryButtonStyle())
                            }
                        }
                    }

                    if manager.trustedTestBuildEnabled() {
                        IrisSectionCard {
                            CardHeader(
                                title: "Trusted test build",
                                subtitle: "This build uses a controlled relay set and is intended for trusted testing only."
                            )
                        }
                    }

                    IrisSectionCard {
                        CardHeader(
                            title: "Support",
                            subtitle: "Capture a support bundle or inspect current build metadata."
                        )
                        Text("Build \(manager.buildSummaryText())")
                            .font(.system(.body, design: .rounded))
                            .foregroundStyle(palette.textPrimary)
                        Text("Relay set \(manager.relaySetIdText())")
                            .font(.system(.body, design: .rounded))
                            .foregroundStyle(palette.muted)

                        Button("Share support bundle") {
                            shareText = manager.supportBundleJson()
                        }
                        .buttonStyle(IrisPrimaryButtonStyle())
                        .accessibilityIdentifier("myProfileShareSupportBundleButton")

                        Button("Copy support bundle") {
                            manager.copyToClipboard(manager.supportBundleJson())
                        }
                        .buttonStyle(IrisSecondaryButtonStyle())
                        .accessibilityIdentifier("myProfileCopySupportBundleButton")

                        Button("Reset app state", role: .destructive) {
                            dismiss()
                            manager.resetAppState()
                        }
                        .buttonStyle(IrisSecondaryButtonStyle())
                        .accessibilityIdentifier("myProfileResetStateButton")
                    }

                    Button("Logout", role: .destructive) {
                        manager.logout()
                        dismiss()
                    }
                    .buttonStyle(IrisPrimaryButtonStyle())
                    .accessibilityIdentifier("myProfileLogoutButton")
                }
            }
            .navigationTitle("Profile")
            .navigationBarTitleDisplayMode(.inline)
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

private struct BackgroundFill: View {
    @Environment(\.irisPalette) private var palette

    var body: some View {
        LinearGradient(
            colors: [
                palette.background,
                palette.background,
                palette.panelAlt.opacity(0.28)
            ],
            startPoint: .top,
            endPoint: .bottom
        )
        .ignoresSafeArea()
    }
}

private struct ToastView: View {
    @Environment(\.irisPalette) private var palette
    let text: String

    var body: some View {
        Text(text)
            .font(.system(.subheadline, design: .rounded, weight: .semibold))
            .foregroundStyle(palette.textPrimary)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(
                Capsule(style: .continuous)
                    .fill(palette.panel)
                    .overlay(Capsule(style: .continuous).stroke(palette.border, lineWidth: 1))
            )
    }
}

private struct LoadingOverlay: View {
    @Environment(\.irisPalette) private var palette

    var body: some View {
        ZStack {
            palette.background.opacity(0.4).ignoresSafeArea()
            VStack(spacing: 14) {
                ProgressView()
                    .progressViewStyle(.circular)
                Text("Loading")
                    .font(.system(.headline, design: .rounded, weight: .semibold))
                    .foregroundStyle(palette.textPrimary)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 22)
            .background(
                RoundedRectangle(cornerRadius: 24, style: .continuous)
                    .fill(palette.panel)
                    .overlay(
                        RoundedRectangle(cornerRadius: 24, style: .continuous)
                            .stroke(palette.border, lineWidth: 1)
                    )
            )
        }
    }
}

private struct CardHeader: View {
    @Environment(\.irisPalette) private var palette
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.system(.title3, design: .rounded, weight: .bold))
                .foregroundStyle(palette.textPrimary)
            Text(subtitle)
                .font(.system(.body, design: .rounded))
                .foregroundStyle(palette.muted)
        }
    }
}

private struct MonoValue: View {
    @Environment(\.irisPalette) private var palette
    let label: String
    let value: String
    let identifier: String?

    init(label: String, value: String, identifier: String? = nil) {
        self.label = label
        self.value = value
        self.identifier = identifier
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.system(.caption, design: .rounded, weight: .semibold))
                .foregroundStyle(palette.muted)
            if let identifier {
                Text(value)
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(palette.textPrimary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier(identifier)
            } else {
                Text(value)
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(palette.textPrimary)
                    .textSelection(.enabled)
            }
        }
    }
}

private struct SelectedMemberChip: View {
    @Environment(\.irisPalette) private var palette
    let title: String
    let subtitle: String?
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(.caption, design: .rounded, weight: .semibold))
                    .lineLimit(1)
                if let subtitle {
                    Text(subtitle)
                        .font(.system(.caption2, design: .monospaced, weight: .medium))
                        .foregroundStyle(palette.muted)
                        .lineLimit(1)
                }
            }
            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .bold))
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("memberChipRemove")
        }
        .foregroundStyle(palette.textPrimary)
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .background(
            Capsule(style: .continuous)
                .fill(palette.panel)
                .overlay(Capsule(style: .continuous).stroke(palette.border, lineWidth: 1))
        )
    }
}

private struct ChatMessageRow: View {
    @Environment(\.irisPalette) private var palette
    let message: ChatMessageSnapshot
    let chatKind: ChatKind
    let showDayChip: Bool
    let isFirstInCluster: Bool
    let isLastInCluster: Bool

    var body: some View {
        VStack(spacing: 0) {
            if showDayChip {
                HStack {
                    Spacer()
                    IrisDayChip(text: irisTimelineDay(message.createdAtSecs))
                    Spacer()
                }
                .padding(.vertical, 14)
            }

            VStack(
                alignment: message.isOutgoing ? .trailing : .leading,
                spacing: 6
            ) {
                if chatKind == .group && !message.isOutgoing && isFirstInCluster {
                    Text(message.author)
                        .font(.system(.caption, design: .rounded, weight: .semibold))
                        .foregroundStyle(palette.muted)
                }

                Text(message.body)
                    .font(.system(.body, design: .rounded))
                    .foregroundStyle(message.isOutgoing ? palette.onBubbleMine : palette.onBubbleTheirs)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 11)
                    .background(
                        RoundedRectangle(cornerRadius: 22, style: .continuous)
                            .fill(message.isOutgoing ? palette.bubbleMine : palette.bubbleTheirs)
                    )
                    .accessibilityIdentifier("chatMessage-\(message.id)")

                HStack(spacing: 6) {
                    Text(irisMessageClock(message.createdAtSecs))
                        .font(.system(.caption2, design: .rounded, weight: .medium))
                    if message.isOutgoing {
                        Text(irisDeliveryLabel(message.delivery))
                            .font(.system(.caption2, design: .rounded, weight: .medium))
                    }
                }
                .foregroundStyle(palette.muted)
            }
            .frame(maxWidth: .infinity, alignment: message.isOutgoing ? .trailing : .leading)
            .padding(.top, isFirstInCluster ? 10 : 4)
            .padding(.bottom, isLastInCluster ? 10 : 0)
        }
    }
}

private struct FlowWrap<Content: View>: View {
    let spacing: CGFloat
    let lineSpacing: CGFloat
    let content: () -> Content

    init(
        spacing: CGFloat = 8,
        lineSpacing: CGFloat = 8,
        @ViewBuilder content: @escaping () -> Content
    ) {
        self.spacing = spacing
        self.lineSpacing = lineSpacing
        self.content = content
    }

    var body: some View {
        ViewThatFits {
            HStack(alignment: .top, spacing: spacing, content: content)
            VStack(alignment: .leading, spacing: lineSpacing, content: content)
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
