//===----------------------------------------------------------------------===//
// swiftformat-wasm shim: minimal C ABI around the SwiftFormat library.
//
// ABI:
//   swiftformat_alloc(len: i32) -> ptr: i32                 host allocates input scratch
//   swiftformat_free(ptr, len)                              release an alloc'd buffer
//   swiftformat_format(srcPtr, srcLen, cfgPtr, cfgLen) -> packed: i64
//       >= 0: success, output length in bytes (UTF-8, via out_ptr/out_len)
//       <  0: error; message UTF-8 via err_ptr/err_len
//             -1 = config decode error, -2 = SwiftFormatError, -3 = other
//   swiftformat_out_ptr() -> ptr: i32
//   swiftformat_out_len() -> len: i32
//   swiftformat_err_ptr() -> ptr: i32
//   swiftformat_err_len() -> len: i32
//   swiftformat_wasm_version() -> version: i32
//
// cfg JSON: swift-format `Configuration` JSON; cfgLen == 0 uses defaults.
//
// Syntax errors: 603.0.0 only reports parse failures through
// `parsingDiagnosticHandler` (without one, swift-format formats garbage
// best-effort). We collect diagnostics, record ERROR-severity ones, and let
// the library throw `fileContainsInvalidSyntax` for those; warnings alone
// neither throw nor fail the call.
//
// No Foundation FileManager/URL FS access on the format path: the formatter
// is handed `nil` for assumingFileURL, so the module only ever needs the
// WASI imports its runtime uses at startup.
//===----------------------------------------------------------------------===//

import Foundation
import SwiftDiagnostics
import SwiftFormat

/// Accumulating TextOutputStream.
struct ByteStream: TextOutputStream {
  var bytes: [UInt8] = []
  mutating func write(_ string: String) {
    bytes.append(contentsOf: string.utf8)
  }
}

@main
struct ShimMain {
  static func main() {}  // host drives everything through the exported C ABI
}

nonisolated(unsafe) var outBuffer: [UInt8] = []
nonisolated(unsafe) var errBuffer: [UInt8] = []

@_cdecl("swiftformat_alloc")
public func swiftformat_alloc(_ len: Int32) -> UnsafeMutableRawPointer? {
  return UnsafeMutableRawPointer.allocate(byteCount: Int(len), alignment: 16)
}

@_cdecl("swiftformat_free")
public func swiftformat_free(_ ptr: UnsafeMutableRawPointer?, _ len: Int32) {
  ptr?.deallocate()
}

@_cdecl("swiftformat_format")
public func swiftformat_format(
  _ srcPtr: UnsafeRawPointer, _ srcLen: Int32,
  _ cfgPtr: UnsafeRawPointer?, _ cfgLen: Int32
) -> Int64 {
  // Decode configuration (or defaults). Exercises JSONDecoder on every call.
  var configuration: Configuration
  if let cfgPtr, cfgLen > 0 {
    let cfgData = Data(bytes: cfgPtr, count: Int(cfgLen))
    do {
      configuration = try Configuration(data: cfgData)
    } catch {
      errBuffer = Array("config decode error: \(error)".utf8)
      return -1
    }
  } else {
    configuration = Configuration()
  }

  let source = String(decoding: UnsafeBufferPointer(start: srcPtr.bindMemory(to: UInt8.self, capacity: Int(srcLen)), count: Int(srcLen)), as: UTF8.self)

  var diagnostics: [String] = []
  var hasErrors = false
  let formatter = SwiftFormatter(configuration: configuration)
  var stream = ByteStream()
  do {
    try formatter.format(
      source: source,
      assumingFileURL: nil,
      selection: .infinite,
      to: &stream,
      parsingDiagnosticHandler: { diag, _ in
        if diagnostics.count < 8 {
          diagnostics.append("\(diag.diagMessage.severity): \(diag.message)")
        }
        if diag.diagMessage.severity == .error { hasErrors = true }
      }
    )
  } catch {
    var message = "format error: \(error)"
    if !diagnostics.isEmpty {
      message += " [" + diagnostics.joined(separator: "; ") + "]"
    }
    errBuffer = Array(message.utf8)
    if error is SwiftFormatError { return -2 }
    return -3
  }
  outBuffer = stream.bytes
  return Int64(outBuffer.count)
}

@_cdecl("swiftformat_out_ptr")
public func swiftformat_out_ptr() -> UnsafeRawPointer? {
  return outBuffer.withUnsafeBufferPointer { UnsafeRawPointer($0.baseAddress) }
}

@_cdecl("swiftformat_out_len")
public func swiftformat_out_len() -> Int32 {
  return Int32(outBuffer.count)
}

@_cdecl("swiftformat_err_ptr")
public func swiftformat_err_ptr() -> UnsafeRawPointer? {
  return errBuffer.withUnsafeBufferPointer { UnsafeRawPointer($0.baseAddress) }
}

@_cdecl("swiftformat_err_len")
public func swiftformat_err_len() -> Int32 {
  return Int32(errBuffer.count)
}

@_cdecl("swiftformat_wasm_version")
public func swiftformat_wasm_version() -> Int32 {
  return 603_000  // marker for swift-format 603.0.0
}
