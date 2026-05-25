import ProjectDescription

let project = Project(
    name: "TestApp",
    packages: [
        .local(path: ".."),
    ],
    targets: [
        .target(
            name: "TestApp",
            destinations: .iOS,
            product: .app,
            bundleId: "dev.wvb.testapp",
            deploymentTargets: .iOS("14.0"),
            infoPlist: .extendingDefault(with: [
                "UILaunchScreen": .dictionary([:]),
            ]),
            sources: ["TestApp/**/*.swift"],
            resources: [
                "TestApp/Assets.xcassets",
                .folderReference(path: "./assets"),
            ],
            dependencies: [
                .package(product: "WebViewBundleLibrary"),
                .package(product: "WebViewBundleWebView"),
            ],
            settings: .settings(base: [
                "SWIFT_VERSION": "5.0",
            ])
        ),
    ]
)
