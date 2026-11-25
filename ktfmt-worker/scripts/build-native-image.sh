#!/usr/bin/env bash
# Build the ktfmt-worker GraalVM native image without Maven.
#
# Requires a GraalVM JDK 25 with native-image on PATH. Reproduces ktfmt v0.65's
# own native image build flags (buildSrc/src/main/kotlin/ktfmt.native-image.gradle.kts
# in the ktfmt repository), including --static-nolibc on Linux/macOS, which the
# pom.xml profile omits because it is not portable to Windows.
#
# Output: ktfmt-worker/ktfmt-worker(.exe)
set -euo pipefail
cd "$(dirname "$0")/.."

KTFMT_VERSION=0.65
JAR=libs/ktfmt-${KTFMT_VERSION}-with-dependencies.jar

if [ ! -f "$JAR" ]; then
	./scripts/fetch-libs.sh --no-mvn
fi

command -v native-image >/dev/null 2>&1 || {
	echo "native-image not found on PATH; install GraalVM (distribution: graalvm-community, JDK 25)." >&2
	exit 1
}

# The @TargetClass substitution compiles against GraalVM's SVM annotations,
# which live in the org.graalvm.sdk:nativeimage Maven artifact (not in the
# GraalVM distribution jars since GraalVM 23.1). Download it if absent.
GRAALVM_VERSION=25.0.1
SDK_JAR=libs/nativeimage-${GRAALVM_VERSION}.jar
if [ ! -f "$SDK_JAR" ]; then
	mkdir -p libs
	curl -sSL -o "$SDK_JAR" \
		"https://repo1.maven.org/maven2/org/graalvm/sdk/nativeimage/${GRAALVM_VERSION}/nativeimage-${GRAALVM_VERSION}.jar"
fi

BUILD=build/native
rm -rf "$BUILD"
mkdir -p "$BUILD/classes" "$BUILD/resources"

# Windows (git-bash) needs ';' as the classpath separator.
SEP=":"
case "$(uname -s)" in
	MINGW* | MSYS* | CYGWIN*) SEP=";" ;;
esac

SOURCES=$(find src/main/java -name '*.java')
javac --release 17 -cp "$JAR$SEP$SDK_JAR" -d "$BUILD/classes" $SOURCES
cp -r src/main/resources/. "$BUILD/resources/"
jar --create --file "$BUILD/ktfmt-worker.jar" \
	--main-class dev.fama.ktfmtworker.Main \
	-C "$BUILD/classes" . -C "$BUILD/resources" .

EXTRA_ARGS=()
case "$(uname -s)" in
	Linux | Darwin)
		# --static-nolibc links a static zlib; skip it where only shared zlib
		# exists (e.g. Arch ships no libz.a).
		if find /usr/lib /lib -maxdepth 1 -name 'libz.a' 2>/dev/null | grep -q .; then
			EXTRA_ARGS+=(--static-nolibc)
		fi
		;;
esac

native-image \
	-O3 -march=compatibility \
	--no-fallback --future-defaults=all \
	--link-at-build-time=org.jetbrains.kotlinx.ktfmt \
	--add-opens=java.base/java.util=ALL-UNNAMED \
	--gc=serial \
	-H:+ReportExceptionStackTraces \
	-H:-UseContainerSupport \
	-R:+InstallSegfaultHandler \
	-H:-ReduceImplicitExceptionStackTraceInformation \
	-J--enable-native-access=ALL-UNNAMED \
	-J--illegal-native-access=allow \
	-J--sun-misc-unsafe-memory-access=allow \
	"${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}" \
	-H:Name=ktfmt-worker \
	-cp "$BUILD/ktfmt-worker.jar$SEP$JAR" \
	dev.fama.ktfmtworker.Main

echo "Built ktfmt-worker/ktfmt-worker"
