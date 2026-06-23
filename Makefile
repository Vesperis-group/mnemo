# Makefile : raccourcis de développement et d'installation pour mnemo.
# Usage : make <cible>

CARGO ?= cargo
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN    := target/release/mnemo
MUSL_TARGET := x86_64-unknown-linux-musl

# Scripts shell du dépôt (analysés par bash -n et ShellCheck).
SHELL_SCRIPTS := \
	scripts/install.sh \
	scripts/uninstall.sh \
	scripts/package-release.sh \
	scripts/generate-sbom.sh \
	scripts/checksums-release.sh \
	scripts/sign-release.sh \
	scripts/lib/bashrc.sh

.DEFAULT_GOAL := build
.PHONY: build release test lint fmt check audit security-check release-check \
        shellcheck actionlint sast security-full \
        sbom sign-check install uninstall man clean help

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

## shellcheck : analyse statique des scripts shell (ShellCheck requis)
shellcheck:
	@if ! command -v shellcheck >/dev/null 2>&1; then \
		echo "!! shellcheck absent - https://github.com/koalaman/shellcheck (ou: apt-get install shellcheck)"; exit 1; \
	fi
	@echo "==> ShellCheck"
	shellcheck $(SHELL_SCRIPTS)

## actionlint : lint statique des workflows GitHub Actions (actionlint requis)
actionlint:
	@if ! command -v actionlint >/dev/null 2>&1; then \
		echo "!! actionlint absent - https://github.com/rhysd/actionlint (go install / binaire épinglé)"; exit 1; \
	fi
	@echo "==> actionlint"
	actionlint

## sast : analyse statique locale (clippy strict + shellcheck + actionlint)
# CodeQL (SAST Rust approfondi) tourne en CI via .github/workflows/codeql.yml.
sast: 
	@echo "==> SAST local (clippy + shellcheck + actionlint)"
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(MAKE) shellcheck
	$(MAKE) actionlint

## security-full : porte de sécurité complète (audit supply chain + lint infra)
# Échoue si un outil requis est absent (gate volontairement strict, contrairement
# à `audit` qui est tolérant). Outils attendus : cargo-audit, cargo-deny,
# cargo-machete, gitleaks, shellcheck, actionlint.
security-full:
	@echo "==> Security-full : audit supply chain + ShellCheck + actionlint"
	@if ! command -v cargo-audit >/dev/null 2>&1; then \
		echo "!! cargo-audit absent - 'cargo install cargo-audit'"; exit 1; fi
	@if ! command -v cargo-deny >/dev/null 2>&1; then \
		echo "!! cargo-deny absent - 'cargo install cargo-deny'"; exit 1; fi
	@if ! command -v cargo-machete >/dev/null 2>&1; then \
		echo "!! cargo-machete absent - 'cargo install cargo-machete'"; exit 1; fi
	@if ! command -v gitleaks >/dev/null 2>&1; then \
		echo "!! gitleaks absent - https://github.com/gitleaks/gitleaks"; exit 1; fi
	@echo "--- cargo audit"
	$(CARGO) audit
	@echo "--- cargo deny check"
	$(CARGO) deny check
	@echo "--- cargo machete"
	$(CARGO) machete
	@echo "--- gitleaks detect"
	gitleaks detect --source . --redact --verbose
	$(MAKE) shellcheck
	$(MAKE) actionlint

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
	bash -n scripts/generate-sbom.sh
	bash -n scripts/checksums-release.sh
	bash -n scripts/sign-release.sh
	npm ci
	npx release-it --dry-run --ci --config release-it.json \
		--no-git.requireCleanWorkingDir --no-git.requireBranch

## sbom : génère le SBOM CycloneDX (nécessite cargo-cyclonedx épinglé)
sbom:
	@if ! cargo cyclonedx --version >/dev/null 2>&1; then \
		echo "!! cargo-cyclonedx absent - 'cargo install cargo-cyclonedx --version 0.5.9 --locked'"; exit 1; \
	fi
	bash scripts/generate-sbom.sh

## sign-check : vérifie l'outillage de signature/provenance (sans signer)
# La signature keyless réelle nécessite l'OIDC du job CI `publish` ; en local on
# se limite à contrôler la présence de cosign et la syntaxe du script de signature.
sign-check:
	@if command -v cosign >/dev/null 2>&1; then \
		echo "--- cosign disponible"; cosign version; \
	else echo "!! cosign absent - voir le workflow CI (job publish) pour la version épinglée"; fi
	bash -n scripts/sign-release.sh
	@echo "OK : la signature/provenance keyless réelle s'exécute uniquement en CI (OIDC)."

## install : compile et installe via scripts/install.sh
install:
	bash scripts/install.sh

## uninstall : désinstalle via scripts/uninstall.sh
uninstall:
	bash scripts/uninstall.sh

## man : affiche la page de manuel locale (docs/man/mnemo.1)
man:
	man ./docs/man/mnemo.1

## clean : nettoie les artefacts de compilation
clean:
	$(CARGO) clean

## help : liste les cibles disponibles
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## /  /'
