#!/usr/bin/env bash
# Download the ktfmt with-dependencies jar into libs/ and (unless --no-mvn is
# given) install it into the local Maven repository under the coordinates the
# pom.xml expects. CI and the native-image script can read libs/ directly.
#
# Usage: scripts/fetch-libs.sh [--no-mvn]
set -euo pipefail
cd "$(dirname "$0")/.."

KTFMT_VERSION=0.65
JAR=libs/ktfmt-${KTFMT_VERSION}-with-dependencies.jar
URL="https://github.com/Kotlin/ktfmt/releases/download/v${KTFMT_VERSION}/ktfmt-${KTFMT_VERSION}-with-dependencies.jar"

if [ ! -f "$JAR" ]; then
	mkdir -p libs
	echo "Downloading $URL"
	curl -sSL -o "$JAR" "$URL"
fi

if [ "${1:-}" = "--no-mvn" ]; then
	exit 0
fi

if ! command -v mvn >/dev/null 2>&1; then
	echo "mvn not found; skipping installation into the local Maven repository." >&2
	echo "Pass --no-mvn to silence this warning." >&2
	exit 0
fi

mvn -q install:install-file \
	-Dfile="$JAR" \
	-DgroupId=org.jetbrains.kotlinx \
	-DartifactId=ktfmt-with-dependencies \
	-Dversion="$KTFMT_VERSION" \
	-Dpackaging=jar
