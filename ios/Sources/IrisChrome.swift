import SwiftUI

enum IrisLayout {
    #if canImport(AppKit)
    static let usesDesktopChrome = true
    static let chromeMaxWidth: CGFloat = 1240
    static let scrollMaxWidth: CGFloat = 1100
    static let chatMaxWidth: CGFloat = 1240
    static let topBarCornerRadius: CGFloat = 18
    static let sectionCornerRadius: CGFloat = 22
    static let inputCornerRadius: CGFloat = 14
    static let buttonCornerRadius: CGFloat = 16
    static let compactButtonCornerRadius: CGFloat = 14
    static let pillCornerRadius: CGFloat = 14
    static let contentHorizontalPadding: CGFloat = 22
    static let contentTopPadding: CGFloat = 10
    static let contentBottomPadding: CGFloat = 32
    #else
    static let usesDesktopChrome = false
    static let chromeMaxWidth: CGFloat? = nil
    static let scrollMaxWidth: CGFloat? = nil
    static let chatMaxWidth: CGFloat? = nil
    static let topBarCornerRadius: CGFloat = 24
    static let sectionCornerRadius: CGFloat = 26
    static let inputCornerRadius: CGFloat = 18
    static let buttonCornerRadius: CGFloat = 999
    static let compactButtonCornerRadius: CGFloat = 999
    static let pillCornerRadius: CGFloat = 999
    static let contentHorizontalPadding: CGFloat = 16
    static let contentTopPadding: CGFloat = 8
    static let contentBottomPadding: CGFloat = 28
    #endif
}

struct IrisPalette {
    let background: Color
    let panel: Color
    let panelAlt: Color
    let border: Color
    let toolbar: Color
    let bubbleMine: Color
    let bubbleTheirs: Color
    let accent: Color
    let accentAlt: Color
    let textPrimary: Color
    let muted: Color
    let onAccent: Color
    let onBubbleMine: Color
    let onBubbleTheirs: Color

    static let light = IrisPalette(
        background: Color(hex: 0xFFFFFF),
        panel: Color(hex: 0xF7F9FA),
        panelAlt: Color(hex: 0xE1E8ED),
        border: Color.black.opacity(0.08),
        toolbar: Color(hex: 0xF7F9FA).opacity(0.96),
        bubbleMine: Color(hex: 0x0F1419),
        bubbleTheirs: Color(hex: 0xF7F9FA),
        accent: Color(hex: 0x0EA5E9),
        accentAlt: Color(hex: 0xDB8216),
        textPrimary: Color(hex: 0x0F1419),
        muted: Color(hex: 0x536471),
        onAccent: .white,
        onBubbleMine: .white,
        onBubbleTheirs: Color(hex: 0x0F1419)
    )

    static let dark = IrisPalette(
        background: Color(hex: 0x030918),
        panel: Color(hex: 0x1B1C48),
        panelAlt: Color(hex: 0x121212),
        border: Color.white.opacity(0.16),
        toolbar: Color(hex: 0x030918).opacity(0.92),
        bubbleMine: Color(hex: 0x702ACE),
        bubbleTheirs: Color(hex: 0x2A3C5E),
        accent: Color(hex: 0x702ACE),
        accentAlt: Color(hex: 0xDB8216),
        textPrimary: .white,
        muted: Color(hex: 0xD1D5DB),
        onAccent: .white,
        onBubbleMine: .white,
        onBubbleTheirs: .white
    )
}

private struct IrisPaletteKey: EnvironmentKey {
    static let defaultValue = IrisPalette.light
}

extension EnvironmentValues {
    var irisPalette: IrisPalette {
        get { self[IrisPaletteKey.self] }
        set { self[IrisPaletteKey.self] = newValue }
    }
}

struct IrisTheme<Content: View>: View {
    @Environment(\.colorScheme) private var colorScheme
    let content: () -> Content

    init(@ViewBuilder content: @escaping () -> Content) {
        self.content = content
    }

    var body: some View {
        let palette = colorScheme == .dark ? IrisPalette.dark : IrisPalette.light
        content()
            .environment(\.irisPalette, palette)
            .tint(palette.accent)
            .preferredColorScheme(nil)
    }
}

private extension Color {
    init(hex: UInt32) {
        let red = Double((hex >> 16) & 0xFF) / 255.0
        let green = Double((hex >> 8) & 0xFF) / 255.0
        let blue = Double(hex & 0xFF) / 255.0
        self.init(.sRGB, red: red, green: green, blue: blue, opacity: 1)
    }
}

struct IrisTopBar: View {
    @Environment(\.irisPalette) private var palette

    let title: String
    let canGoBack: Bool
    let onBack: () -> Void
    let leading: AnyView
    let trailing: AnyView

    init(
        title: String,
        canGoBack: Bool,
        onBack: @escaping () -> Void,
        leading: AnyView = AnyView(EmptyView()),
        trailing: AnyView = AnyView(EmptyView())
    ) {
        self.title = title
        self.canGoBack = canGoBack
        self.onBack = onBack
        self.leading = leading
        self.trailing = trailing
    }

    var body: some View {
        HStack(spacing: 10) {
            Color.clear
                .frame(width: 0, height: 0)
                .accessibilityIdentifier("navigationTopBar")

            if canGoBack {
                Button(action: onBack) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 20, weight: .bold))
                        .foregroundStyle(palette.textPrimary)
                        .frame(width: 44, height: 44)
                        .background(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .fill(palette.panel.opacity(0.64))
                        )
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Back")
                .accessibilityIdentifier("navigationBackButton")
            } else {
                leading
                    .frame(minWidth: 44, alignment: .leading)
            }

            Text(title)
                .font(.system(.title3, design: .rounded, weight: .bold))
                .lineLimit(1)
                .foregroundStyle(palette.textPrimary)
                .frame(maxWidth: .infinity, alignment: .center)

            trailing
                .frame(minWidth: 44, alignment: .trailing)
        }
        .padding(.horizontal, IrisLayout.usesDesktopChrome ? 12 : 14)
        .padding(.vertical, IrisLayout.usesDesktopChrome ? 6 : 8)
        .background(
            Rectangle()
                .fill(palette.toolbar)
        )
        .frame(maxWidth: IrisLayout.chromeMaxWidth)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 0)
        .padding(.top, IrisLayout.usesDesktopChrome ? 0 : 4)
        .padding(.bottom, 0)
    }
}

struct IrisSectionCard<Content: View>: View {
    @Environment(\.irisPalette) private var palette

    let accent: Bool
    let content: () -> Content

    init(
        accent: Bool = false,
        @ViewBuilder content: @escaping () -> Content
    ) {
        self.accent = accent
        self.content = content
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14, content: content)
            .padding(18)
            .background(
                RoundedRectangle(cornerRadius: IrisLayout.sectionCornerRadius, style: .continuous)
                    .fill(accent ? palette.panelAlt : palette.panel)
                    .overlay(
                        RoundedRectangle(cornerRadius: IrisLayout.sectionCornerRadius, style: .continuous)
                            .stroke(accent ? palette.accent.opacity(0.24) : palette.border, lineWidth: 1)
                    )
            )
            .shadow(
                color: Color.black.opacity(IrisLayout.usesDesktopChrome ? 0.04 : 0),
                radius: IrisLayout.usesDesktopChrome ? 22 : 0,
                y: IrisLayout.usesDesktopChrome ? 12 : 0
            )
    }
}

struct IrisScrollScreen<Content: View>: View {
    let content: () -> Content

    init(@ViewBuilder content: @escaping () -> Content) {
        self.content = content
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16, content: content)
                .frame(maxWidth: IrisLayout.scrollMaxWidth, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.horizontal, IrisLayout.contentHorizontalPadding)
                .padding(.top, IrisLayout.contentTopPadding)
                .padding(.bottom, IrisLayout.contentBottomPadding)
        }
        .scrollIndicators(.hidden)
    }
}

struct IrisAdaptiveColumns<Leading: View, Trailing: View>: View {
    let alignment: VerticalAlignment
    let spacing: CGFloat
    let leading: () -> Leading
    let trailing: () -> Trailing

    init(
        alignment: VerticalAlignment = .top,
        spacing: CGFloat = 16,
        @ViewBuilder leading: @escaping () -> Leading,
        @ViewBuilder trailing: @escaping () -> Trailing
    ) {
        self.alignment = alignment
        self.spacing = spacing
        self.leading = leading
        self.trailing = trailing
    }

    var body: some View {
        Group {
            if IrisLayout.usesDesktopChrome {
                HStack(alignment: alignment, spacing: spacing) {
                    leading()
                        .frame(maxWidth: .infinity, alignment: .leading)
                    trailing()
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack(alignment: .leading, spacing: spacing) {
                    leading()
                    trailing()
                }
            }
        }
    }
}

struct IrisAvatar: View {
    @Environment(\.irisPalette) private var palette

    let label: String
    let size: CGFloat
    let emphasize: Bool

    init(label: String, size: CGFloat = 42, emphasize: Bool = false) {
        self.label = label
        self.size = size
        self.emphasize = emphasize
    }

    var body: some View {
        ZStack {
            Circle()
                .fill(emphasize ? palette.accent : palette.panelAlt)
                .overlay(Circle().stroke(palette.border, lineWidth: 1))

            Text(String((label.trimmingCharacters(in: .whitespacesAndNewlines).first ?? "?")).uppercased())
                .font(.system(size: size * 0.42, weight: .bold, design: .rounded))
                .foregroundStyle(emphasize ? palette.onAccent : palette.textPrimary)
        }
        .frame(width: size, height: size)
    }
}

struct IrisPrimaryButtonStyle: ButtonStyle {
    @Environment(\.irisPalette) private var palette
    let compact: Bool

    init(compact: Bool = false) {
        self.compact = compact
    }

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(compact ? .subheadline : .body, design: .rounded, weight: .semibold))
            .foregroundStyle(palette.onAccent)
            .padding(.horizontal, compact ? 14 : 18)
            .padding(.vertical, compact ? 10 : 14)
            .frame(maxWidth: compact ? nil : .infinity)
            .background(
                Group {
                    if IrisLayout.usesDesktopChrome {
                        RoundedRectangle(
                            cornerRadius: compact ? IrisLayout.compactButtonCornerRadius : IrisLayout.buttonCornerRadius,
                            style: .continuous
                        )
                        .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
                    } else {
                        Capsule(style: .continuous)
                            .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
                    }
                }
            )
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
            .animation(.easeOut(duration: 0.14), value: configuration.isPressed)
    }
}

struct IrisSecondaryButtonStyle: ButtonStyle {
    @Environment(\.irisPalette) private var palette
    let compact: Bool

    init(compact: Bool = false) {
        self.compact = compact
    }

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(compact ? .subheadline : .body, design: .rounded, weight: .semibold))
            .foregroundStyle(palette.textPrimary)
            .padding(.horizontal, compact ? 14 : 18)
            .padding(.vertical, compact ? 10 : 14)
            .frame(maxWidth: compact ? nil : .infinity)
            .background(
                Group {
                    if IrisLayout.usesDesktopChrome {
                        RoundedRectangle(
                            cornerRadius: compact ? IrisLayout.compactButtonCornerRadius : IrisLayout.buttonCornerRadius,
                            style: .continuous
                        )
                        .fill(palette.panel.opacity(0.72))
                    } else {
                        Capsule(style: .continuous)
                            .fill(palette.panel)
                    }
                }
            )
            .opacity(configuration.isPressed ? 0.9 : 1)
    }
}

struct IrisInputFieldModifier: ViewModifier {
    @Environment(\.irisPalette) private var palette

    func body(content: Content) -> some View {
        content
            .textFieldStyle(.plain)
            .font(.system(.body, design: .rounded))
            .padding(.horizontal, 14)
            .padding(.vertical, 13)
            .background(
                RoundedRectangle(cornerRadius: IrisLayout.inputCornerRadius, style: .continuous)
                    .fill(palette.panel.opacity(IrisLayout.usesDesktopChrome ? 0.62 : 1))
            )
    }
}

extension View {
    func irisInputField() -> some View {
        modifier(IrisInputFieldModifier())
    }
}

struct IrisInfoPill: View {
    @Environment(\.irisPalette) private var palette

    let text: String
    let tint: Color?

    init(_ text: String, tint: Color? = nil) {
        self.text = text
        self.tint = tint
    }

    var body: some View {
        Text(text)
            .font(.system(.caption, design: .rounded, weight: .semibold))
            .foregroundStyle(tint ?? palette.muted)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: IrisLayout.pillCornerRadius, style: .continuous)
                    .fill((tint ?? palette.panelAlt).opacity(0.14))
            )
    }
}

struct IrisChatRow: View {
    @Environment(\.irisPalette) private var palette

    let title: String
    let preview: String
    let subtitle: String?
    let timeLabel: String?
    let unreadCount: UInt64
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(alignment: .top, spacing: 14) {
                IrisAvatar(label: title, emphasize: unreadCount > 0)

                VStack(alignment: .leading, spacing: 5) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(title)
                            .font(.system(.headline, design: .rounded, weight: .semibold))
                            .foregroundStyle(palette.textPrimary)
                            .lineLimit(1)

                        Spacer(minLength: 8)

                        if let timeLabel, !timeLabel.isEmpty {
                            Text(timeLabel)
                                .font(.system(.caption, design: .rounded, weight: .medium))
                                .foregroundStyle(palette.muted)
                                .lineLimit(1)
                        }
                    }

                    Text(preview)
                        .font(.system(.subheadline, design: .rounded))
                        .foregroundStyle(palette.muted)
                        .lineLimit(2)

                    if let subtitle, !subtitle.isEmpty {
                        Text(subtitle)
                            .font(.system(.caption, design: .rounded, weight: .medium))
                            .foregroundStyle(palette.muted)
                            .lineLimit(1)
                    }
                }

                if unreadCount > 0 {
                    Text("\(unreadCount)")
                        .font(.system(.caption, design: .rounded, weight: .bold))
                        .foregroundStyle(palette.onAccent)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 5)
                        .background(Capsule(style: .continuous).fill(palette.accent))
                }
            }
            .padding(.vertical, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

struct IrisDayChip: View {
    @Environment(\.irisPalette) private var palette
    let text: String

    var body: some View {
        Text(text)
            .font(.system(.caption, design: .rounded, weight: .semibold))
            .foregroundStyle(palette.muted)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(
                RoundedRectangle(cornerRadius: IrisLayout.pillCornerRadius, style: .continuous)
                    .fill(palette.panel.opacity(0.64))
            )
    }
}

struct IrisComposerBar: View {
    @Environment(\.irisPalette) private var palette

    @Binding var draft: String
    let placeholder: String
    let isSending: Bool
    let onSend: () -> Void

    private var canSend: Bool {
        !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !isSending
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            Color.clear
                .frame(width: 0, height: 0)
                .accessibilityIdentifier("chatComposerBar")

            TextField(placeholder, text: $draft)
                .irisDraftInputModifiers()
                .irisInputField()
                .irisDesktopSubmit(submitDraft)
                .accessibilityIdentifier("chatMessageInput")

            Button(action: submitDraft) {
                Image(systemName: isSending ? "ellipsis.circle.fill" : "paperplane.fill")
                    .font(.system(size: 18, weight: .bold))
                    .frame(width: IrisLayout.usesDesktopChrome ? 52 : 46, height: 46)
            }
            .buttonStyle(IrisPrimaryCircleButtonStyle())
            .disabled(!canSend)
            .accessibilityIdentifier("chatSendButton")
        }
        .padding(.horizontal, IrisLayout.usesDesktopChrome ? 16 : IrisLayout.contentHorizontalPadding)
        .padding(.top, 10)
        .padding(.bottom, 12)
        .background(
            Rectangle()
                .fill(palette.toolbar)
        )
        .frame(maxWidth: .infinity)
    }

    private func submitDraft() {
        guard canSend else {
            return
        }
        onSend()
    }
}

private struct IrisPrimaryCircleButtonStyle: ButtonStyle {
    @Environment(\.irisPalette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(palette.onAccent)
            .background(
                Group {
                    if IrisLayout.usesDesktopChrome {
                        RoundedRectangle(cornerRadius: IrisLayout.buttonCornerRadius, style: .continuous)
                            .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
                            .frame(width: 52, height: 46)
                    } else {
                        Circle()
                            .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
                            .frame(width: 46, height: 46)
                    }
                }
            )
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.14), value: configuration.isPressed)
    }
}

func irisRelativeTime(_ secs: UInt64?) -> String? {
    guard let secs else { return nil }
    let date = Date(timeIntervalSince1970: TimeInterval(secs))
    return RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
}

func irisTimelineDay(_ secs: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(secs))
    let calendar = Calendar.current
    if calendar.isDateInToday(date) {
        return "Today"
    }
    if calendar.isDateInYesterday(date) {
        return "Yesterday"
    }
    return irisDayFormatter.string(from: date)
}

func irisMessageClock(_ secs: UInt64) -> String {
    irisTimeFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(secs)))
}

func irisSameTimelineDay(_ lhs: UInt64, _ rhs: UInt64) -> Bool {
    Calendar.current.isDate(
        Date(timeIntervalSince1970: TimeInterval(lhs)),
        inSameDayAs: Date(timeIntervalSince1970: TimeInterval(rhs))
    )
}

func irisDeliveryLabel(_ delivery: DeliveryState) -> String {
    switch delivery {
    case .pending:
        return "Pending"
    case .sent:
        return "Sent"
    case .received:
        return "Received"
    case .failed:
        return "Failed"
    }
}

private let irisDayFormatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.dateFormat = "EEE, MMM d"
    return formatter
}()

private let irisTimeFormatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.dateStyle = .none
    formatter.timeStyle = .short
    return formatter
}()
