set -e

REPO="AkaraChen/fama"
INSTALL_DIR="/usr/local/bin"

get_github_token() {
	if ! command -v gh >/dev/null 2>&1; then
		return 1
	fi

	if ! gh auth status >/dev/null 2>&1; then
		return 1
	fi

	gh auth token 2>/dev/null
}

main() {
	GITHUB_TOKEN=$(get_github_token) || true
	if [ -n "$GITHUB_TOKEN" ]; then
		echo "Using GitHub CLI authentication"
		AUTH_HEADER="Authorization: token $GITHUB_TOKEN"
	else
		AUTH_HEADER=""
	fi

	os=$(uname -s | tr '[:upper:]' '[:lower:]')
	arch=$(uname -m)

	case "$os" in
	linux) os="unknown-linux-gnu" ;;
	darwin) os="apple-darwin" ;;
	*)
		echo "Unsupported OS: $os"
		echo "Windows users: download from https://github.com/$REPO/releases and add to PATH manually"
		exit 1
		;;
	esac

	case "$arch" in
	x86_64 | amd64) arch="x86_64" ;;
	arm64 | aarch64) arch="aarch64" ;;
	*)
		echo "Unsupported architecture: $arch"
		exit 1
		;;
	esac

	target="${arch}-${os}"

	echo "Detecting system... $target"

	if [ -n "$AUTH_HEADER" ]; then
		tag=$(curl -fsSL -H "$AUTH_HEADER" "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
	else
		tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
	fi

	if [ -z "$tag" ]; then
		echo "Failed to fetch latest release"
		exit 1
	fi

	echo "Latest version: $tag"

	filename="fama-${tag}-${target}.tar.gz"
	url="https://github.com/$REPO/releases/download/${tag}/${filename}"

	tmpdir=$(mktemp -d)
	trap 'rm -rf "$tmpdir"' EXIT

	echo "Downloading $filename..."
	curl -fsSL "$url" -o "$tmpdir/fama.tar.gz"

	echo "Extracting..."
	tar -xzf "$tmpdir/fama.tar.gz" -C "$tmpdir"

	echo "Installing to $INSTALL_DIR..."
	if [ -w "$INSTALL_DIR" ]; then
		mv "$tmpdir/fama" "$INSTALL_DIR/fama"
	else
		sudo mv "$tmpdir/fama" "$INSTALL_DIR/fama"
	fi

	chmod +x "$INSTALL_DIR/fama"

	echo ""
	echo "Done! fama installed to $INSTALL_DIR/fama"
	echo "Run 'fama --help' to get started"
}

main
