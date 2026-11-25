# ktfmt-worker

A persistent Kotlin formatting server wrapping [ktfmt](https://github.com/Kotlin/ktfmt)
v0.65's public API, shipped as a GraalVM native image. fama spawns one worker per
thread for an entire repository run and drives it over a framed stdin/stdout
protocol — no temp files, no per-file process spawns.

ktfmt's CLI has two costs per file: process startup (~6–12 ms for the native
build, ~1–2 s on the JVM) and, before ktfmt 0.65's native images, a JVM spin-up.
For a repository with hundreds of `.kt`/`.kts` files this dominates formatting
time. The worker pays startup once and then serves every file over a socket-free
pipe protocol.

## Wire protocol

All integers are unsigned 32-bit big-endian. `stdin` carries requests, `stdout`
carries responses; the worker writes nothing else to stdout (diagnostics go to
stderr).

```
request   := len:u32 payload:u8[len]
payload   := kind:u8 source:u8[len-1]
             kind 0 = regular .kt source
             kind 1 = Kotlin script (.kts) source
response  := status:u8 len:u32 payload:u8[len]
             status 0 = ok,    payload := formatted source (UTF-8)
             status 1 = error, payload := error message (worker stays alive)
             status 2 = fatal, payload := error message (worker exits)
```

- `source` is UTF-8 Kotlin source without a trailing NUL.
- The `kind` byte exists because ktfmt parses `.kts` sources with a script PSI
  file type; feeding `build.gradle.kts` content through the regular file type
  fails with `ParseError: Expecting a top level declaration`.
- On status 1 (per-file format error) the worker stays alive and protocol-synced;
  the client may send the next request immediately.
- On status 2 (fatal) the worker writes the response and exits; the client must
  respawn. Fatals are reserved for virtual-machine errors (`OutOfMemoryError`,
  `InternalError`, …). Everything else — including `StackOverflowError` and all
  parse failures — is caught per request and reported as status 1.
- EOF on stdin terminates the worker cleanly (exit 0).
- Request frames larger than 256 MiB are treated as a protocol desync: the
  worker responds with status 2 and exits.

## Command line

```
ktfmt-worker [--kotlinlang-style|--google-style|--meta-style] [--editorconfig <path>]
```

- Style defaults to `--meta-style`. fama passes `--kotlinlang-style` (this is the
  mapping fama's central configuration has always used for Kotlin).
- `--editorconfig <path>` points at one `.editorconfig` file (fama writes a
  single generated one per run and passes the path at spawn). The file is
  resolved with ktfmt's own `EditorConfigResolver`, exactly like
  `ktfmt --enable-editorconfig`: keys such as `max_line_length` and `indent_size`
  override the base style. `--editorconfig` and a style flag may be combined;
  keys present in the editorconfig win.
- Line endings follow the input (ktfmt preserves CRLF), matching the CLI.

## Building

Prerequisites: a GraalVM JDK 25 (`graalvm-community` distribution; `native-image`
on PATH), `javac`, `jar`.

```sh
./scripts/fetch-libs.sh          # download ktfmt-0.65-with-dependencies.jar into libs/
./scripts/build-native-image.sh  # -> ./ktfmt-worker (native image)
```

The script reproduces the exact build flags of ktfmt v0.65's own native image
release build (see `buildSrc/src/main/kotlin/ktfmt.native-image.gradle.kts` in
the ktfmt repository). The vendored pieces that make ktfmt native-image-friendly
live under `src/main/`:

- `src/main/resources/META-INF/native-image/org.jetbrains.kotlinx.ktfmt/reachability-metadata.json`
  — ktfmt's reflection/reachability metadata (vendored from ktfmt v0.65).
- `src/main/resources/META-INF/native-image/org.jetbrains.kotlinx.ktfmt/native-image.properties`
  — ktfmt's build-time/run-time initialization rules (vendored from ktfmt v0.65).
- `src/main/java/org/jetbrains/kotlinx/ktfmt/nativeImage/KotlinCoreEnvironmentCompanion.java`
  — ktfmt's SVM substitution stubbing IntelliJ extension-point loading (vendored
  from ktfmt v0.65).

The worker's own protocol surface needs no reflection config.

One extra quirk of the shaded jar: it carries jline's
`META-INF/native-image/.../native-image.properties` but not the JSON files they
reference, which would abort any native-image build. The stub configs under
`src/main/resources/META-INF/native-image/org.jline/` satisfy those references;
they are intentionally empty because the worker never touches jline (only the
ktfmt CLI uses it).

### Maven

`pom.xml` builds the same thing:

```sh
./scripts/fetch-libs.sh            # downloads libs/ jar + installs into the local repo
mvn package                        # JVM jar
mvn -Pnative package               # native image -> target/ktfmt-worker
```

The `native` profile mirrors the script but omits `--static-nolibc` (unix-only;
it cannot be activated per-OS cleanly in one pom).

## Running under the JVM (fallback mode)

Without a native image the same class runs on any JDK 17+:

```sh
java -cp build/native/ktfmt-worker.jar:libs/ktfmt-0.65-with-dependencies.jar \
	dev.fama.ktfmtworker.Main --kotlinlang-style
```

The protocol is identical; only startup (~1–2 s once per worker) is slower.
fama accepts any executable that speaks the protocol via `FAMA_KTFMT_WORKER`, so
a thin wrapper script around `java -cp …` works there too.

## See also

- `formatters/kotlin-worker/src/lib.rs` — the Rust client (worker discovery,
  per-thread worker pooling, fallback behavior).
- `.github/workflows/ktfmt-worker.yml` — CI builds native images for
  linux-x86_64, macos-aarch64 and windows-x86_64 and attaches them to releases.
