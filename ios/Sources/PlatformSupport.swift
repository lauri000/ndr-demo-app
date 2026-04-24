import SwiftUI

#if canImport(UIKit)
import UIKit

typealias PlatformImage = UIImage

extension Image {
    init(platformImage: PlatformImage) {
        self.init(uiImage: platformImage)
    }
}
#elseif canImport(AppKit)
import AppKit

typealias PlatformImage = NSImage

extension Image {
    init(platformImage: PlatformImage) {
        self.init(nsImage: platformImage)
    }
}
#endif

enum PlatformClipboard {
    static func string() -> String? {
        #if canImport(UIKit)
        UIPasteboard.general.string
        #elseif canImport(AppKit)
        NSPasteboard.general.string(forType: .string)
        #else
        nil
        #endif
    }

    static func setString(_ value: String) {
        #if canImport(UIKit)
        UIPasteboard.general.string = value
        #elseif canImport(AppKit)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(value, forType: .string)
        #endif
    }
}

var irisSupportsQrScanning: Bool {
    #if canImport(UIKit)
    true
    #else
    false
    #endif
}

extension View {
    @ViewBuilder
    func irisIdentifierInputModifiers() -> some View {
        #if canImport(UIKit)
        self
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
        #else
        self
        #endif
    }

    @ViewBuilder
    func irisDraftInputModifiers() -> some View {
        #if canImport(UIKit)
        self
            .textInputAutocapitalization(.sentences)
            .autocorrectionDisabled(false)
        #else
        self
        #endif
    }

    @ViewBuilder
    func irisDesktopSubmit(_ action: @escaping () -> Void) -> some View {
        #if canImport(AppKit)
        self.onSubmit(action)
        #else
        self
        #endif
    }

    @ViewBuilder
    func irisOnChange<Value: Equatable>(
        of value: Value,
        _ action: @escaping (Value) -> Void
    ) -> some View {
        #if canImport(AppKit)
        self.onChange(of: value) { _, newValue in
            action(newValue)
        }
        #else
        self.onChange(of: value, perform: action)
        #endif
    }

    @ViewBuilder
    func irisInteractiveKeyboardDismiss() -> some View {
        #if canImport(UIKit)
        self.scrollDismissesKeyboard(.interactively)
        #else
        self
        #endif
    }

    @ViewBuilder
    func irisInlineTitleDisplayMode() -> some View {
        #if canImport(UIKit)
        self.navigationBarTitleDisplayMode(.inline)
        #else
        self
        #endif
    }

    @ViewBuilder
    func irisOnExitCommand(_ action: @escaping () -> Void) -> some View {
        #if canImport(AppKit)
        self.onExitCommand(perform: action)
        #else
        self
        #endif
    }
}

var irisToolbarTrailingPlacement: ToolbarItemPlacement {
    #if canImport(UIKit)
    .topBarTrailing
    #else
    .automatic
    #endif
}
