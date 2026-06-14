# Makefile - raccourcis de développement et d'installation pour mnemo.
# Usage : make <cible>

CARGO ?= cargo
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN    := target/release/mnemo
MUSL_TARGET := x86_64-unknown-linux-musl

.DEFAULT_GOAL := build
.PHONY: build release test lint fmt check audit security-check release-check \
        install uninstall clean help

## build : compilation en mode debug
build:
	$(CARGO) build

## release : compilation optimisée
release:
	$(CARGO) build --release

## test : exécute la suite de tests
test:
	$(CARGO) test

## lint : formatage (vérif) + clippy strict
lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings

## fmt : applique le formatage
fmt:
	$(CARGO) fmt --all

## check : contrôle rapide avant commit (fmt + clippy + tests)
check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(CARGO) test

## audit : outils DevSecOps (ignorés proprement si absents)
audit:
	@echo "==> Audit de sécurité et chaîne d'approvisionnement"
	@if command -v cargo-audit >/dev/null 2>&1; then \
		echo "--- cargo audit"; $(CARGO) audit; \
	else echo "!! cargo-audit absent - 'cargo install cargo-audit'"; fi
	@if command -v cargo-deny >/dev/null 2>&1; then \
		echo "--- cargo deny check"; $(CARGO) deny check; \
	else echo "!! cargo-deny absent - 'cargo install cargo-deny'"; fi
	@if command -v cargo-machete >/dev/null 2>&1; then \
		echo "--- cargo machete"; $(CARGO) machete; \
	else echo "!! cargo-machete absent - 'cargo install cargo-machete'"; fi
	@if command -v gitleaks >/dev/null 2>&1; then \
		echo "--- gitleaks detect"; gitleaks detect --source . --no-banner; \
	else echo "!! gitleaks absent - https://github.com/gitleaks/gitleaks"; fi

## security-check : alias de `audit`
security-check: audit

## release-check : porte de qualité complète avant release
release-check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(CARGO) test
	$(CARGO) build --release
	$(CARGO) build --release --target $(MUSL_TARGET)
	bash -n scripts/install.sh
	bash -n scripts/uninstall.sh
	bash -n scripts/lib/bashrc.sh
	bash -n scripts/package-release.sh
	npm ci
	npx release-it --dry-run --ci --config release-it.json \
		--no-git.requireCleanWorkingDir --no-git.requireBranch

## install : compile et installe via scripts/install.sh
install:
	bash scripts/install.sh

## uninstall : désinstalle via scripts/uninstall.sh
uninstall:
	bash scripts/uninstall.sh

## clean : nettoie les artefacts de compilation
clean:
	$(CARGO) clean

## help : liste les cibles disponibles
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## /  /'
