.PHONY: build release clean

build:
	cargo build

release:
	cargo build --release

clean:
	cargo clean

# Pre-generated concept content lives in content/*.md and is embedded at compile time.
# To regenerate: open Claude Code and ask it to regenerate content for the desired sections.
# Files are committed to the repo — no API calls needed at runtime.
