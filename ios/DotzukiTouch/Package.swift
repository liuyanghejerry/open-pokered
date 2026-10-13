// swift-tools-version: 5.7
import PackageDescription

let package = Package(
    name: "DotzukiTouch",
    platforms: [.iOS(.v15)],
    products: [.library(name: "DotzukiTouch", targets: ["DotzukiTouch"])],
    targets: [.target(
        name: "DotzukiTouch",
        path: ".",
        exclude: ["tests", "README.md", "README.zh-CN.md"],
        sources: ["ios/DotzukiTouchOverlay.mm"],
        publicHeadersPath: "ios/include",
        cxxSettings: [.headerSearchPath("include")],
        linkerSettings: [.linkedFramework("UIKit")]
    ), .testTarget(name: "DotzukiTouchTests", dependencies: ["DotzukiTouch"], path: "tests/ios")],
    cxxLanguageStandard: .cxx17
)
