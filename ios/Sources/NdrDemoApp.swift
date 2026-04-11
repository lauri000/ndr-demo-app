import SwiftUI

@main
struct NdrDemoApp: App {
    @StateObject private var manager = AppManager()

    var body: some Scene {
        WindowGroup {
            RootView(manager: manager)
        }
    }
}
