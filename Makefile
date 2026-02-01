.PHONY: all build release clean test

# Default target
all: release

# Development build
build:
	cargo build

# Release build
release:
	cargo build --release

# Clean build artifacts
clean:
	cargo clean
	rm -f formatters/shfmt/go/libshformatter.a
	rm -f formatters/shfmt/go/libshformatter.h
	rm -f formatters/shfmt/go/libshformatter.dylib
	rm -f formatters/shfmt/go/libshformatter.so

# Run tests
test:
	cargo test

# Install the binary
install: release
	@echo "Installing fama to /usr/local/bin..."
	@sudo cp target/release/fama /usr/local/bin/fama

# Development build
dev:
	cargo build
