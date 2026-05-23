// swift-tools-version: 5.9
import PackageDescription

let package = Package(
  name: "WebViewBundleLocalTests",
  platforms: [.macOS(.v12), .iOS(.v14)],
  products: [
    .library(name: "WebViewBundleLibrary", targets: ["WebViewBundleLibrary"])
  ],
  targets: [
    .binaryTarget(
      name: "WebViewBundleFFI",
      path: "WebViewBundleFFI.xcframework"
    ),
    .target(
      name: "WebViewBundleLibrary",
      dependencies: [.target(name: "WebViewBundleFFI")],
      path: "src",
      linkerSettings: [
        .linkedFramework("SystemConfiguration"),
        .linkedFramework("Security"),
        .linkedFramework("CoreFoundation")
      ],
    ),
    .testTarget(
      name: "WebViewBundleLibTests",
      dependencies: ["WebViewBundleLibrary"],
      path: "tests",
    )
  ]
)
