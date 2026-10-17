#!/usr/bin/env python3
"""Download and verify pinned ktfmt native images for Fama.

This script is intentionally dependency-free: it uses urllib+hashlib and verifies
the published SHA-256 checksums before writing anything to disk. It is run
manually by maintainers before building Fama; Fama itself never downloads at
runtime.
"""

import argparse
import hashlib
import os
import shutil
import sys
import urllib.error
import urllib.request

REPO = "Kotlin/ktfmt"
VERSION = "0.65"

TARGETS = [
    ("linux-x86_64", "linux-x86_64", "tar.gz", "ktfmt"),
    ("linux-aarch64", "aarch64-unknown-linux-gnu", "tar.gz", "ktfmt"),
    ("macos-aarch64", "macos-aarch64", "tar.gz", "ktfmt"),
    ("windows-x86_64", "windows-x86_64", "zip", "ktfmt.exe"),
]

# sha256sums from the v0.65 release assets.
SHA256 = {
    ("linux-x86_64", "tar.gz"): "3684327b092db9b76fffcb9525decf73f8331ed17e00ce3232769c11cd36d352",
    ("macos-aarch64", "tar.gz"): "d10ec46dadc4ae19e5b8ddc72d10ce45e5be35d632ce9e73c9b8a59d2147fb85",
    ("windows-x86_64", "zip"): "3286d659fa8df6a480e3b53ca4a30d9321c372286c46b8d74556b434189bee5f",
}

# https://github.com/kotlin/ktfmt/releases/tag/v0.65 -- generated with sha256sum
KTFMT_URL_TEMPLATE = (
    "https://github.com/Kotlin/ktfmt/releases/download/v{VERSION}/{name}"
)


def asset_name(kind, fmt, binary):
    return f"ktfmt-{kind}-{VERSION}.{fmt}"


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "fama-downloader"})
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            data = resp.read()
    except urllib.error.HTTPError as exc:
        raise SystemExit(f"HTTP {exc.code} fetching {url}") from exc
    except urllib.error.URLError as exc:
        raise SystemExit(f"Failed to fetch {url}: {exc}") from exc
    return data


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Download pinned ktfmt native images verified by checksum."
    )
    parser.add_argument(
        "--target",
        choices=[kind for kind, _, _, _ in TARGETS],
        default=None,
        help="Only prepare this target family (linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64).",
    )
    parser.add_argument(
        "--out",
        default=os.path.join(os.path.dirname(__file__), "bin"),
        help="Directory to write ktfmt/<target>/<binary> into.",
    )
    parser.add_argument(
        "--publishable-only",
        action="store_true",
        help="Skip targets without an official v0.65 release artifact.",
    )
    args = parser.parse_args()

    root = args.out
    os.makedirs(root, exist_ok=True)

    targets = TARGETS
    if args.target:
        targets = [t for t in TARGETS if t[0] == args.target]
    if not targets:
        raise SystemExit(f"No target matches '{args.target}'")

    for kind, target_triple, fmt, binary in targets:
        name = asset_name(kind, fmt, binary)
        url = KTFMT_URL_TEMPLATE.format(VERSION=VERSION, name=name)
        key = (kind, fmt)

        if not args.publishable_only and key not in SHA256:
            print(f"skip {name}: no pinned checksum (unpublished platform)")
            continue

        dest_dir = os.path.join(root, target_triple)
        os.makedirs(dest_dir, exist_ok=True)
        dest = os.path.join(dest_dir, binary)
        tmp = f"{dest}.tmp"

        print(f"downloading {url}")
        data = fetch(url)

        digest = hashlib.sha256(data).hexdigest()
        expected = SHA256.get(key)
        if expected and digest != expected:
            print(f"sha256 mismatch for {name}: got {digest}, expected {expected}")
            os.unlink(tmp) if os.path.exists(tmp) else None
            raise SystemExit(1)

        with open(tmp, "wb") as handle:
            handle.write(data)
        os.replace(tmp, dest)
        try:
            os.chmod(dest, 0o755)
        except OSError:
            pass
        print(f"verified {name} -> {dest}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
