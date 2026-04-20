import SwiftUI

@main
struct IrisChatApp: App {
    @StateObject private var manager = AppManager()

    var body: some Scene {
        WindowGroup {
            RootView(manager: manager)
        }
    }
}
