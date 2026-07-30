BINARY_NAME = lazyTrino
TARGET_DIR = target/release

.PHONY: all build dev clean run help

all: build

build:
	@echo "Building release binary..."
	cargo build --release
	@cp $(TARGET_DIR)/$(BINARY_NAME) ./$(BINARY_NAME)
	@echo "Binary created at ./$(BINARY_NAME)"

dev:
	@echo "Building debug binary..."
	cargo build
	@cp target/debug/$(BINARY_NAME) ./$(BINARY_NAME)
	@echo "Binary created at ./$(BINARY_NAME)"

clean:
	cargo clean
	rm -f ./$(BINARY_NAME)

run: build
	./$(BINARY_NAME)

help:
	@echo "Available targets:"
	@echo "  make build  - Build release binary and copy to project root (default)"
	@echo "  make dev    - Build debug binary and copy to project root"
	@echo "  make run    - Build and execute the root binary"
	@echo "  make clean  - Clean build artifacts and remove root binary"
