import SwiftUI

@main
struct IrisChatApp: App {
    @UIApplicationDelegateAdaptor(IrisPushAppDelegate.self) private var appDelegate
    @StateObject private var manager = AppManager()

    var body: some Scene {
        WindowGroup {
            RootView(manager: manager)
        }
    }
}
