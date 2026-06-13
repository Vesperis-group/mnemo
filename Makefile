# Makefile — raccourcis de développement et d'installation pour mnemo.
# Usage : make <cible>

CARGO ?= cargo
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN    := target/release/mnemo

.DEFAULT_GOAL := build
.PHONY: build release test lint fmt install uninstall clean help

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
