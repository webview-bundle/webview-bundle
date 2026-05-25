// swift-tools-version: 5.9
import PackageDescription

let package = Package(
  name: "WebViewBundleLocalTests",
  platforms: [.macOS(.v12), .iOS(.v14)],
  products: [
    .library(name: "WebViewBundleLibrary", targets: ["WebViewBundleLibrary"]),
    .library(name: "WebViewBundleWebView", targets: ["WebViewBundleWebView"])
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
    .target(
      name: "WebViewBundleWebView",
      dependencies: ["WebViewBundleLibrary"],
      path: "Sources/WebViewBundleWebView",
      exclude: ["README.md"],
      linkerSettings: [
        .linkedFramework("WebKit")
      ],
    ),
    .testTarget(
      name: "WebViewBundleLibTests",
      dependencies: ["WebViewBundleLibrary"],
      path: "tests",
    )
  ]
)
