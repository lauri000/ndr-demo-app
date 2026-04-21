import SwiftUI

@main
struct IrisChatMacApp: App {
    @StateObject private var manager = AppManager()

    var body: some Scene {
        WindowGroup {
            RootView(manager: manager)
                .frame(minWidth: 860, minHeight: 620)
        }
        .defaultSize(width: 1180, height: 820)
        .windowResizability(.automatic)
    }
}
