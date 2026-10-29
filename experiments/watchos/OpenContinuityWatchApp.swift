import SwiftUI

@main
struct OpenContinuityWatchApp: App {
    @Environment(\.scenePhase) private var scenePhase
    @StateObject private var bluetooth = WatchBluetoothManager()

    var body: some Scene {
        WindowGroup {
            ContentView(bluetooth: bluetooth)
                .onChange(of: scenePhase) { _, phase in
                    if phase != .active { bluetooth.applicationDidSuspend() }
                }
        }
    }
}
