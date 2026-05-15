set shell := ["bash", "-uc"]

ci: lint audit test

lint: fmt clippy typos

fmt:
    cargo fmt --check

check *args="":
    cargo check --all-targets {{args}}

clippy *args="":
    cargo clippy --all-targets {{args}}

test *args="":
    cargo test {{args}}

doc *args="":
    cargo doc --no-deps {{args}}

audit:
    cargo audit

typos:
    typos
