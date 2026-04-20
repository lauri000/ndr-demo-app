import SwiftUI
import UIKit

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
        HStack(spacing: 12) {
            Color.clear
                .frame(width: 0, height: 0)
                .accessibilityIdentifier("navigationTopBar")

            if canGoBack {
                Button(action: onBack) {
                    Label("Back", systemImage: "chevron.left")
                        .font(.system(.body, design: .rounded, weight: .semibold))
                }
                .buttonStyle(IrisSecondaryButtonStyle(compact: true))
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
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .fill(palette.toolbar)
                .overlay(
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .stroke(palette.border, lineWidth: 1)
                )
        )
        .padding(.horizontal, 12)
        .padding(.top, 8)
        .padding(.bottom, 10)
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
                RoundedRectangle(cornerRadius: 26, style: .continuous)
                    .fill(accent ? palette.panelAlt : palette.panel)
                    .overlay(
                        RoundedRectangle(cornerRadius: 26, style: .continuous)
                            .stroke(accent ? palette.accent.opacity(0.24) : palette.border, lineWidth: 1)
                    )
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
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.top, 8)
                .padding(.bottom, 28)
        }
        .scrollIndicators(.hidden)
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
                Capsule(style: .continuous)
                    .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
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
                Capsule(style: .continuous)
                    .fill(palette.panel)
                    .overlay(
                        Capsule(style: .continuous)
                            .stroke(palette.border, lineWidth: 1)
                    )
            )
            .opacity(configuration.isPressed ? 0.9 : 1)
    }
}

struct IrisInputFieldModifier: ViewModifier {
    @Environment(\.irisPalette) private var palette

    func body(content: Content) -> some View {
        content
            .font(.system(.body, design: .rounded))
            .padding(.horizontal, 14)
            .padding(.vertical, 13)
            .background(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(palette.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .stroke(palette.border, lineWidth: 1)
                    )
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
                Capsule(style: .continuous)
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
                Capsule(style: .continuous)
                    .fill(palette.panel)
                    .overlay(Capsule(style: .continuous).stroke(palette.border, lineWidth: 1))
            )
    }
}

struct IrisComposerBar: View {
    @Environment(\.irisPalette) private var palette

    @Binding var draft: String
    let placeholder: String
    let isSending: Bool
    let onSend: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            Color.clear
                .frame(width: 0, height: 0)
                .accessibilityIdentifier("chatComposerBar")

            TextField(placeholder, text: $draft)
                .textInputAutocapitalization(.sentences)
                .autocorrectionDisabled(false)
                .irisInputField()
                .accessibilityIdentifier("chatMessageInput")

            Button(action: onSend) {
                Image(systemName: isSending ? "ellipsis.circle.fill" : "paperplane.fill")
                    .font(.system(size: 18, weight: .bold))
                    .frame(width: 46, height: 46)
            }
            .buttonStyle(IrisPrimaryCircleButtonStyle())
            .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSending)
            .accessibilityIdentifier("chatSendButton")
        }
        .padding(.horizontal, 16)
        .padding(.top, 14)
        .padding(.bottom, 12)
        .background(
            Rectangle()
                .fill(palette.toolbar)
                .overlay(alignment: .top) {
                    Divider().overlay(palette.border)
                }
        )
    }
}

private struct IrisPrimaryCircleButtonStyle: ButtonStyle {
    @Environment(\.irisPalette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(palette.onAccent)
            .background(
                Circle()
                    .fill(palette.accent.opacity(configuration.isPressed ? 0.86 : 1))
                    .frame(width: 46, height: 46)
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
