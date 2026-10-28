// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "OpenContinuityWatchProtocol",
    platforms: [.watchOS(.v10), .macOS(.v13)],
    products: [.library(name: "OpenContinuityProtocol", targets: ["OpenContinuityProtocol"])],
    targets: [
        .target(name: "OpenContinuityProtocol", path: ".", exclude: ["Package.swift", "Tests", "README.md", "OpenContinuityWatchApp.swift", "ContentView.swift", "WatchBluetoothManager.swift", "GattIdentifiers.swift", "GattResearchState.swift"] , sources: ["GattWireCodec.swift"]),
        .testTarget(name: "OpenContinuityWatchTests", dependencies: ["OpenContinuityProtocol"], path: "Tests/OpenContinuityWatchTests", resources: [.copy("Fixtures")])
    ]
)
