// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "swiftformat-wasm",
  dependencies: [
    .package(path: "../swift-format"),
    .package(url: "https://github.com/swiftlang/swift-syntax.git", from: "603.0.0")
  ],
  targets: [
    .executableTarget(
      name: "swiftformat_wasm",
      dependencies: [
        .product(name: "SwiftFormat", package: "swift-format"),
        .product(name: "SwiftDiagnostics", package: "swift-syntax")
      ],
      swiftSettings: [
        .swiftLanguageMode(.v5)
      ],
      linkerSettings: [
        .unsafeFlags([
          "-Xlinker", "--export=swiftformat_format",
          "-Xlinker", "--export=swiftformat_out_ptr",
          "-Xlinker", "--export=swiftformat_out_len",
          "-Xlinker", "--export=swiftformat_err_ptr",
          "-Xlinker", "--export=swiftformat_err_len",
          "-Xlinker", "--export=swiftformat_alloc",
          "-Xlinker", "--export=swiftformat_free",
          "-Xlinker", "--export=swiftformat_wasm_version",
        ])
      ]
    )
  ]
)
