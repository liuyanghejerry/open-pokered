.PHONY: build release debug clean serve help

help:
	@echo "Pokémon Red Web Build Commands"
	@echo ""
	@echo "  make build      - Build optimized WASM package (release)"
	@echo "  make release    - Build optimized WASM package (release)"
	@echo "  make debug      - Build WASM package with debug info"
	@echo "  make clean      - Remove build artifacts"
	@echo "  make serve      - Build and start local dev server"
	@echo ""

build: release

release:
	@./build-web.sh release

debug:
	@./build-web.sh debug

clean:
	@rm -rf pkg
	@echo "Build artifacts removed"

serve:
	@./dev-server.sh