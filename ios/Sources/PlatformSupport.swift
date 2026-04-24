import SwiftUI

#if canImport(UIKit)
import UIKit
import WebKit

typealias PlatformImage = UIImage

extension Image {
    init(platformImage: PlatformImage) {
        self.init(uiImage: platformImage)
    }
}

struct IrisAnimatedImageDataView: UIViewRepresentable {
    let data: Data

    func makeUIView(context: Context) -> WKWebView {
        let webView = WKWebView(frame: .zero)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.scrollView.isScrollEnabled = false
        webView.scrollView.bounces = false
        load(data, in: webView)
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        load(data, in: webView)
    }

    private func load(_ data: Data, in webView: WKWebView) {
        webView.loadHTMLString(irisAnimatedImageHTML(data: data), baseURL: nil)
    }
}
#elseif canImport(AppKit)
import AppKit
import WebKit

typealias PlatformImage = NSImage

extension Image {
    init(platformImage: PlatformImage) {
        self.init(nsImage: platformImage)
    }
}

struct IrisAnimatedImageDataView: NSViewRepresentable {
    let data: Data

    func makeNSView(context: Context) -> WKWebView {
        let webView = WKWebView(frame: .zero)
        webView.setValue(false, forKey: "drawsBackground")
        load(data, in: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        load(data, in: webView)
    }

    private func load(_ data: Data, in webView: WKWebView) {
        webView.loadHTMLString(irisAnimatedImageHTML(data: data), baseURL: nil)
    }
}
#endif

private func irisAnimatedImageHTML(data: Data) -> String {
    let encoded = data.base64EncodedString()
    return """
    <!doctype html>
    <html>
    <head>
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1">
    <style>
    html, body {
      margin: 0;
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: transparent;
    }
    body {
      display: flex;
      align-items: center;
      justify-content: center;
    }
    img {
      width: 100%;
      height: 100%;
      object-fit: contain;
    }
    </style>
    </head>
    <body><img src="data:image/gif;base64,\(encoded)" alt=""></body>
    </html>
    """
}

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

    @ViewBuilder
    func irisOnEscapeKey(_ action: @escaping () -> Void) -> some View {
        #if canImport(AppKit)
        self.background(IrisEscapeKeyHandler(action: action).frame(width: 0, height: 0))
        #else
        self
        #endif
    }
}

#if canImport(AppKit)
private struct IrisEscapeKeyHandler: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> IrisEscapeKeyView {
        let view = IrisEscapeKeyView()
        view.action = action
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
        return view
    }

    func updateNSView(_ view: IrisEscapeKeyView, context: Context) {
        view.action = action
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
    }
}

private final class IrisEscapeKeyView: NSView {
    var action: (() -> Void)?

    override var acceptsFirstResponder: Bool {
        true
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            action?()
        } else {
            super.keyDown(with: event)
        }
    }
}
#endif

var irisToolbarTrailingPlacement: ToolbarItemPlacement {
    #if canImport(UIKit)
    .topBarTrailing
    #else
    .automatic
    #endif
}
