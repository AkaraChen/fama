//===----------------------------------------------------------------------===//
// swiftformat-wasm shim: minimal C ABI around the SwiftFormat library.
//
// ABI:
//   swiftformat_alloc(len: i32) -> ptr: i32                 host allocates input scratch
//   swiftformat_free(ptr, len)                              release an alloc'd buffer
//   swiftformat_set_config(lineLen, useTabs, indentWidth)   configure cached formatter once
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
// Config: hosts can either call `swiftformat_set_config` once after `_start`
// and pass `cfgLen == 0` on every format call (avoiding JSON allocation and
// JSONDecoder), or pass a swift-format `Configuration` JSON payload (which is
// also cached across identical calls).
//
// Syntax errors: 603.0.0 only reports parse failures through
// `parsingDiagnosticHandler` (without one, swift-format formats garbage
// best-effort). We collect diagnostics and let the library throw
// `fileContainsInvalidSyntax` for error-severity ones; warnings alone neither
// throw nor fail the call.
//
// No Foundation FileManager/URL FS access on the format path: the formatter
// is handed `nil` for assumingFileURL, so the module only ever needs the
// WASI imports its runtime uses at startup.
//===----------------------------------------------------------------------===//

#if canImport(FoundationEssentials)
import FoundationEssentials
#else
import Foundation
#endif
import SwiftDiagnostics
import SwiftFormat

/// Accumulating TextOutputStream with upfront capacity reservation.
struct ByteStream: TextOutputStream {
  var bytes: [UInt8]

  init(capacity: Int) {
    bytes = []
    bytes.reserveCapacity(capacity)
  }

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
nonisolated(unsafe) var cachedFormatter = SwiftFormatter(configuration: Configuration())
nonisolated(unsafe) var cachedConfigJSON: [UInt8] = []

@_cdecl("swiftformat_alloc")
public func swiftformat_alloc(_ len: Int32) -> UnsafeMutableRawPointer? {
  return UnsafeMutableRawPointer.allocate(byteCount: Int(len), alignment: 16)
}

@_cdecl("swiftformat_free")
public func swiftformat_free(_ ptr: UnsafeMutableRawPointer?, _ len: Int32) {
  ptr?.deallocate()
}

@_cdecl("swiftformat_set_config")
public func swiftformat_set_config(
  _ lineLength: Int32, _ useTabs: Int32, _ indentWidth: Int32
) {
  var configuration = Configuration()
  if lineLength > 0 {
    configuration.lineLength = Int(lineLength)
  }
  if indentWidth > 0 {
    configuration.tabWidth = Int(indentWidth)
    configuration.indentation =
      useTabs != 0 ? .tabs(1) : .spaces(Int(indentWidth))
  }
  cachedFormatter = SwiftFormatter(configuration: configuration)
  cachedConfigJSON = []
}

@_cdecl("swiftformat_format")
public func swiftformat_format(
  _ srcPtr: UnsafeRawPointer, _ srcLen: Int32,
  _ cfgPtr: UnsafeRawPointer?, _ cfgLen: Int32
) -> Int64 {
  let formatter: SwiftFormatter
  if let cfgPtr, cfgLen > 0 {
    let cfgBytes = Array(
      UnsafeBufferPointer(
        start: cfgPtr.bindMemory(to: UInt8.self, capacity: Int(cfgLen)),
        count: Int(cfgLen)
      )
    )
    if cfgBytes == cachedConfigJSON {
      formatter = cachedFormatter
    } else {
      do {
        let configuration = try Configuration(data: Data(cfgBytes))
        formatter = SwiftFormatter(configuration: configuration)
        cachedFormatter = formatter
        cachedConfigJSON = cfgBytes
      } catch {
        errBuffer = Array("config decode error: \(error)".utf8)
        return -1
      }
    }
  } else {
    formatter = cachedFormatter
  }

  let source = String(
    decoding: UnsafeBufferPointer(
      start: srcPtr.bindMemory(to: UInt8.self, capacity: Int(srcLen)),
      count: Int(srcLen)
    ),
    as: UTF8.self
  )

  var diagnostics: [String] = []
  var stream = ByteStream(capacity: Int(srcLen) + 64)
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
  return 603_001  // swift-format 603.0.0 + cached config & ICU stub
}
