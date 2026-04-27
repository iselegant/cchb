.PHONY: help release verify

CARGO_TOML := Cargo.toml
CURRENT_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(CARGO_TOML) | head -n1)

help:
	@echo "Targets:"
	@echo "  make release BUMP=patch    Bump patch (e.g. 0.9.4 -> 0.9.5), commit, tag, push"
	@echo "  make release BUMP=minor    Bump minor (e.g. 0.9.4 -> 0.10.0), commit, tag, push"
	@echo "  make release BUMP=major    Bump major (e.g. 0.9.4 -> 1.0.0),  commit, tag, push"
	@echo "  make verify                Run cargo fmt --check + clippy + test"
	@echo ""
	@echo "Current version: $(CURRENT_VERSION)"

verify:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

release:
	@./scripts/release.sh "$(BUMP)"
