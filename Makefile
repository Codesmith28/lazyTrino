BINARY_NAME = lazyTrino
DIST_DIR    = dist

# ─── Rust targets ────────────────────────────────────────────────────────────
TARGET_LINUX   = x86_64-unknown-linux-gnu
TARGET_DARWIN  = x86_64-apple-darwin
TARGET_WINDOWS = x86_64-pc-windows-gnu

# ─── Detect whether `cross` is available, fall back to cargo ─────────────────
CROSS := $(shell command -v cross 2>/dev/null)
ifdef CROSS
  BUILD_CMD = cross
else
  BUILD_CMD = cargo
endif

# ─── Use rustup-managed cargo for cross-compilation ──────────────────────────
# Homebrew rust (if installed) may shadow cargo on PATH but lacks cross-std.
# `rustup which cargo` always resolves to the active rustup toolchain's cargo.
RUSTUP_CARGO := $(shell rustup which cargo 2>/dev/null || echo cargo)

.PHONY: all build build-linux build-darwin build-windows build-all dev clean run install help

# ─── Default ──────────────────────────────────────────────────────────────────
all: build

# ─── Native build (matches the current host OS) ───────────────────────────────
build:
	@echo "🔨 Building native release binary..."
	@cargo build --release
	@mkdir -p $(DIST_DIR)
	@cp target/release/$(BINARY_NAME)* $(DIST_DIR)/ 2>/dev/null || true
	@echo "✅ Binary → $(DIST_DIR)/$(BINARY_NAME)"

# ─── Linux (x86_64) ───────────────────────────────────────────────────────────
build-linux:
	@echo "🐧 Building for Linux ($(TARGET_LINUX))..."
	@$(BUILD_CMD) build --release --target $(TARGET_LINUX)
	@mkdir -p $(DIST_DIR)
	@cp target/$(TARGET_LINUX)/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-linux-x86_64
	@echo "✅ Binary → $(DIST_DIR)/$(BINARY_NAME)-linux-x86_64"

# ─── macOS (x86_64) ───────────────────────────────────────────────────────────
# Uses cargo-zigbuild to cross-compile via Zig's bundled linker (no osxcross needed).
# Uses RUSTUP_CARGO to avoid Homebrew rust shadowing (which lacks x86_64 cross-std).
# Install deps: brew install zig && cargo install cargo-zigbuild --locked
build-darwin:
	@echo "🍎 Building for macOS ($(TARGET_DARWIN))..."
	@$(RUSTUP_CARGO) zigbuild --release --target $(TARGET_DARWIN)
	@mkdir -p $(DIST_DIR)
	@cp target/$(TARGET_DARWIN)/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-darwin-x86_64
	@echo "✅ Binary → $(DIST_DIR)/$(BINARY_NAME)-darwin-x86_64"

# ─── macOS ARM (Apple Silicon) ────────────────────────────────────────────────
# Uses RUSTUP_CARGO to avoid Homebrew rust shadowing on Apple Silicon hosts.
build-darwin-arm:
	@echo "🍎 Building for macOS ARM (aarch64-apple-darwin)..."
	@$(RUSTUP_CARGO) build --release --target aarch64-apple-darwin
	@mkdir -p $(DIST_DIR)
	@cp target/aarch64-apple-darwin/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-darwin-aarch64
	@echo "✅ Binary → $(DIST_DIR)/$(BINARY_NAME)-darwin-aarch64"

# ─── Windows (x86_64) ─────────────────────────────────────────────────────────
build-windows:
	@echo "🪟 Building for Windows ($(TARGET_WINDOWS))..."
	@$(BUILD_CMD) build --release --target $(TARGET_WINDOWS)
	@mkdir -p $(DIST_DIR)
	@cp target/$(TARGET_WINDOWS)/release/$(BINARY_NAME).exe $(DIST_DIR)/$(BINARY_NAME)-windows-x86_64.exe
	@echo "✅ Binary → $(DIST_DIR)/$(BINARY_NAME)-windows-x86_64.exe"

# ─── Build all platforms at once ──────────────────────────────────────────────
build-all: build-linux build-darwin build-darwin-arm build-windows
	@echo ""
	@echo "📦 All binaries in ./$(DIST_DIR):"
	@ls -lh $(DIST_DIR)/

# ─── Debug build (native) ─────────────────────────────────────────────────────
dev:
	@echo "🔧 Building debug binary..."
	@cargo build
	@cp target/debug/$(BINARY_NAME) ./$(BINARY_NAME)
	@echo "✅ Binary → ./$(BINARY_NAME)"

# ─── Run (native release) ─────────────────────────────────────────────────────
run: build
	./$(BINARY_NAME)

# ─── Install to ~/.local/bin ──────────────────────────────────────────────────
install: build
	@install -Dm755 $(DIST_DIR)/$(BINARY_NAME) ~/.local/bin/$(BINARY_NAME)
	@echo "✅ Installed to ~/.local/bin/$(BINARY_NAME)"

# ─── Clean ────────────────────────────────────────────────────────────────────
clean:
	@cargo clean
	@rm -rf $(DIST_DIR) ./$(BINARY_NAME)
	@echo "🧹 Cleaned build artifacts and dist/"

# ─── Help ─────────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "  $(BINARY_NAME) — Build Targets"
	@echo "  ─────────────────────────────────────────────────────"
	@echo "  make build              Native release binary (auto-detects host OS)"
	@echo "  make build-linux        Cross-compile → Linux  x86_64"
	@echo "  make build-darwin       Cross-compile → macOS  x86_64"
	@echo "  make build-darwin-arm   Cross-compile → macOS  aarch64 (Apple Silicon)"
	@echo "  make build-windows      Cross-compile → Windows x86_64 (.exe)"
	@echo "  make build-all          Build all four platforms at once"
	@echo "  ─────────────────────────────────────────────────────"
	@echo "  make dev                Native debug binary"
	@echo "  make run                Build (native) and run"
	@echo "  make install            Install native binary to ~/.local/bin"
	@echo "  make clean              Remove all build artifacts"
	@echo ""
	@echo "  Cross-compilation uses 'cross' if installed, else 'cargo'."
	@echo "  Install cross:  cargo install cross --locked"
	@echo "  Requires Docker for non-native targets via cross."
	@echo ""
	@echo "  macOS x86_64 / ARM cross-compile (no Docker needed):"
	@echo "    brew install zig"
	@echo "    cargo install cargo-zigbuild --locked"
	@echo "    rustup target add x86_64-apple-darwin   # for build-darwin"
	@echo "    rustup target add aarch64-apple-darwin  # for build-darwin-arm"
	@echo ""
