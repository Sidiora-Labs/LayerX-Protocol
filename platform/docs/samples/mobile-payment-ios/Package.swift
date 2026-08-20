// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "LayerXMobilePayment",
    platforms: [.iOS(.v15), .macOS(.v12)],
    dependencies: [
        .package(path: "../../../integrations/ios"),
        .package(path: "../../../sdk/swift")
    ],
    targets: [
        .executableTarget(
            name: "MobilePayment",
            dependencies: ["LayerXMobile", "LayerXSDK"],
            path: "Sources/MobilePayment"
        )
    ]
)
