const std = @import("std");
const Ast = std.zig.Ast;
const Allocator = std.mem.Allocator;
const Writer = std.Io.Writer;

/// Result struct for format operation
pub const FormatResult = extern struct {
    /// Pointer to formatted output (null-terminated), or null on error
    data: ?[*:0]u8,
    /// Length of the formatted output (excluding null terminator)
    len: usize,
    /// Error message if data is null, otherwise null
    error_msg: ?[*:0]const u8,
};

/// Global allocator for C FFI
var gpa = std.heap.GeneralPurposeAllocator(.{}){};

/// Format Zig source code
/// Returns a FormatResult struct with either the formatted code or an error message
/// The caller must call zig_fmt_free() on the result when done
export fn zig_fmt(source: [*:0]const u8, source_len: usize) FormatResult {
    const allocator = gpa.allocator();

    // Create a slice from the source
    const src_slice: [:0]const u8 = source[0..source_len :0];

    // Parse the source
    var tree = Ast.parse(allocator, src_slice, .zig) catch |err| {
        return .{
            .data = null,
            .len = 0,
            .error_msg = switch (err) {
                error.OutOfMemory => "Out of memory during parsing",
            },
        };
    };
    defer tree.deinit(allocator);

    // Check for parse errors
    if (tree.errors.len > 0) {
        return .{
            .data = null,
            .len = 0,
            .error_msg = "Parse error in source code",
        };
    }

    // Render the formatted output using Writer.Allocating
    var aw: Writer.Allocating = .init(allocator);
    defer aw.deinit();

    Ast.Render.renderTree(allocator, &aw.writer, tree, .{}) catch |err| {
        return .{
            .data = null,
            .len = 0,
            .error_msg = switch (err) {
                error.OutOfMemory => "Out of memory during rendering",
                error.WriteFailed => "Write failed during rendering",
            },
        };
    };

    // Get the output
    const output_list = aw.toArrayList();

    // Allocate result with null terminator
    const result = allocator.allocSentinel(u8, output_list.items.len, 0) catch {
        return .{
            .data = null,
            .len = 0,
            .error_msg = "Out of memory allocating result",
        };
    };
    @memcpy(result, output_list.items);

    // Free the original buffer
    allocator.free(output_list.allocatedSlice());

    return .{
        .data = result.ptr,
        .len = result.len,
        .error_msg = null,
    };
}

/// Free the memory allocated by zig_fmt
export fn zig_fmt_free(result: *FormatResult) void {
    if (result.data) |data| {
        const allocator = gpa.allocator();
        const slice: [:0]u8 = data[0..result.len :0];
        allocator.free(slice);
        result.data = null;
        result.len = 0;
    }
}

/// Get the version of the formatter
export fn zig_fmt_version() [*:0]const u8 {
    return "0.1.0";
}
