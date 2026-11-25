package dev.fama.ktfmtworker;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.EOFException;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import org.jetbrains.kotlinx.ktfmt.cli.EditorConfigResolver;
import org.jetbrains.kotlinx.ktfmt.format.FileType;
import org.jetbrains.kotlinx.ktfmt.format.Formatter;
import org.jetbrains.kotlinx.ktfmt.format.FormattingOptions;
import org.jetbrains.kotlinx.ktfmt.format.KotlinCode;

/**
 * ktfmt-worker — a persistent Kotlin formatting server wrapping ktfmt's public API.
 *
 * <p>fama spawns one worker per thread for an entire repository run and speaks a
 * length-prefixed framing protocol over stdin/stdout. This amortizes ktfmt's
 * startup cost across every file and never spills source to temp files.
 *
 * <h2>Wire protocol</h2>
 *
 * <p>All integers are unsigned 32-bit big-endian.
 *
 * <pre>
 * request   := len:u32 payload:u8[len]
 * payload   := kind:u8 source:u8[len-1]   // UTF-8 Kotlin source
 *              kind 0 = regular .kt source
 *              kind 1 = Kotlin script (.kts) source
 * response  := status:u8 len:u32 payload:u8[len]
 *              status 0 = ok,    payload := formatted source
 *              status 1 = error, payload := error message (worker stays alive)
 *              status 2 = fatal, payload := error message (worker exits)
 * </pre>
 *
 * <p>EOF on stdin shuts the worker down cleanly. Every per-request failure is
 * caught (including {@link StackOverflowError}) and reported as status 1, so one
 * bad file never kills the worker; only virtual-machine errors are reported as
 * fatal.
 */
public final class Main {

	private static final int STATUS_OK = 0;
	private static final int STATUS_FORMAT_ERROR = 1;
	private static final int STATUS_FATAL = 2;

	private static final int KIND_REGULAR = 0;
	private static final int KIND_SCRIPT = 1;

	/**
	 * A request larger than any plausible Kotlin source means the framing is
	 * desynced. The stream cannot be recovered, so the worker reports fatal and exits.
	 */
	private static final int MAX_FRAME_BYTES = 256 * 1024 * 1024;

	private Main() {}

	public static void main(String[] args) {
		FormattingOptions options = optionsFromArgs(args);

		InputStream stdin = new BufferedInputStream(System.in, 1 << 16);
		OutputStream stdout = new BufferedOutputStream(System.out, 1 << 16);

		try {
			run(stdin, stdout, options);
		} catch (IOException ignored) {
			// stdin/stdout broke (parent process died); nothing left to report.
			System.exit(1);
		}
	}

	private static void run(
		InputStream stdin, OutputStream stdout, FormattingOptions options
	) throws IOException {
		while (true) {
			byte[] payload;
			try {
				payload = readFrame(stdin);
			} catch (EOFException eof) {
				return; // EOF on stdin: clean shutdown
			}
			if (payload == null) {
				fatal(stdout, "oversized request frame (protocol desync)");
				return;
			}

			byte kind = payload[0];
			if (kind != KIND_REGULAR && kind != KIND_SCRIPT) {
				fatal(stdout, "unknown request kind " + kind);
				return;
			}
			String source = new String(payload, 1, payload.length - 1, StandardCharsets.UTF_8);

			byte status;
			byte[] response;
			try {
				FileType fileType = kind == KIND_SCRIPT ? FileType.SCRIPT : FileType.REGULAR;
				String formatted = Formatter.format(options, KotlinCode.Companion.from(source, fileType));
				status = STATUS_OK;
				response = formatted.getBytes(StandardCharsets.UTF_8);
			} catch (Throwable throwable) {
				if (isFatal(throwable)) {
					fatal(stdout, describe(throwable));
					return;
				}
				status = STATUS_FORMAT_ERROR;
				response = describe(throwable).getBytes(StandardCharsets.UTF_8);
			}
			writeFrame(stdout, status, response);
			stdout.flush();
		}
	}

	private static FormattingOptions optionsFromArgs(String[] args) {
		FormattingOptions base = Formatter.META_FORMAT;
		Path editorconfig = null;

		for (int i = 0; i < args.length; i++) {
			switch (args[i]) {
				case "--meta-style" -> base = Formatter.META_FORMAT;
				case "--google-style" -> base = Formatter.GOOGLE_FORMAT;
				case "--kotlinlang-style" -> base = Formatter.KOTLINLANG_FORMAT;
				case "--editorconfig" -> {
					if (++i >= args.length) {
						usage("missing path after --editorconfig");
					}
					editorconfig = Path.of(args[i]);
				}
				case "--help", "-h" -> help();
				default -> usage("unknown argument: " + args[i]);
			}
		}

		if (editorconfig == null) {
			return base;
		}

		// Resolve the editorconfig exactly like the ktfmt CLI does: the resolver walks
		// up from the formatted file's directory. The source file itself is never read
		// and does not need to exist, so a virtual file name in the editorconfig's
		// directory is enough to anchor the lookup.
		File directory = editorconfig.getParent() == null
			? new File(".")
			: editorconfig.getParent().toFile();
		File anchor = new File(directory, "__fama_virtual__.kt");
		return EditorConfigResolver.INSTANCE.resolveFormattingOptions(anchor, base);
	}

	/**
	 * Reads one request frame.
	 *
	 * @return the frame payload, or {@code null} on an unrecoverable framing error.
	 * @throws EOFException when stdin ends (clean worker shutdown).
	 */
	private static byte[] readFrame(InputStream in) throws IOException {
		int first = in.read();
		if (first < 0) {
			throw new EOFException();
		}
		int length = first;
		for (int shift = 16; shift >= 0; shift -= 8) {
			int next = in.read();
			if (next < 0) {
				throw new EOFException();
			}
			length = (length << 8) | next;
		}
		if (length > MAX_FRAME_BYTES) {
			return null;
		}

		byte[] payload = new byte[length];
		int read = 0;
		while (read < length) {
			int chunk = in.read(payload, read, length - read);
			if (chunk < 0) {
				throw new EOFException();
			}
			read += chunk;
		}
		return payload;
	}

	private static void writeFrame(OutputStream out, int status, byte[] payload)
		throws IOException {
		out.write(status);
		out.write((payload.length >>> 24) & 0xFF);
		out.write((payload.length >>> 16) & 0xFF);
		out.write((payload.length >>> 8) & 0xFF);
		out.write(payload.length & 0xFF);
		out.write(payload);
	}

	private static void fatal(OutputStream out, String message) {
		try {
			writeFrame(out, STATUS_FATAL, message.getBytes(StandardCharsets.UTF_8));
			out.flush();
		} catch (IOException ignored) {
			// Parent is gone; nothing left to report.
		}
		System.err.println("ktfmt-worker: fatal: " + message);
	}

	/**
	 * Virtual machine errors other than stack overflow leave the process in an
	 * unreliable state: report them as fatal so the client respawns a fresh worker.
	 * Everything else is a per-file failure the worker can survive.
	 */
	private static boolean isFatal(Throwable throwable) {
		return throwable instanceof VirtualMachineError
			&& !(throwable instanceof StackOverflowError);
	}

	private static String describe(Throwable throwable) {
		String message = throwable.getMessage();
		if (message == null || message.isBlank()) {
			message = throwable.toString();
		}
		// Keep one logical error per line for fama's per-file error listing.
		return message.replace('\n', ' ').trim();
	}

	private static void help() {
		System.err.println(
			"usage: ktfmt-worker [--kotlinlang-style|--google-style|--meta-style] [--editorconfig <path>]");
		System.err.println("  Reads framed requests on stdin, writes framed responses on stdout.");
		System.err.println("  Defaults to meta-style. See ktfmt-worker/README.md for the protocol.");
		System.exit(0);
	}

	private static void usage(String error) {
		PrintStream err = System.err;
		if (error != null) {
			err.println("ktfmt-worker: " + error);
		}
		help();
	}
}
