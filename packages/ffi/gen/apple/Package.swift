// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "WebViewBundle",
    platforms: [.macOS(.v12), .iOS(.v14)],
    products: [
        .library(name: "WebViewBundle", targets: ["WebViewBundle"]),
    ],
    targets: [
        .binaryTarget(
            name: "WebViewBundleFFI",
            path: "WebViewBundle.xcframework"
        ),
        .target(
            name: "WebViewBundle",
            dependencies: [.target(name: "WebViewBundleFFI")],
            path: "swift",
            linkerSettings: [
                .linkedFramework("SystemConfiguration"),
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
        .testTarget(
            name: "WebViewBundleTests",
            dependencies: ["WebViewBundle"],
            path: "Tests/Tests"
        ),
    ]
)
