.PHONY: all build release clean test

# Default target
all: release

# Development build
build:
	cargo build

# Release build with dylib fix
release:
	cargo build --release
	@if [ "$(shell uname)" = "Darwin" ]; then \
		install_name_tool -change libshformatter.dylib @rpath/libshformatter.dylib target/release/fama; \
	fi

# Clean build artifacts
clean:
	cargo clean

# Run tests
test:
	cargo test

# Install the binary
install: release
	@echo "Installing fama to /usr/local/bin..."
	@sudo cp target/release/fama /usr/local/bin/fama

# Development build with dylib fix
dev:
	cargo build
	@if [ "$(shell uname)" = "Darwin" ]; then \
		install_name_tool -change libshformatter.dylib @rpath/libshformatter.dylib target/debug/fama; \
	fi
