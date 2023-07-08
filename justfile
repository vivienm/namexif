DEFAULT: fmt check test clippy doc deny typos

fmt:
    cargo fmt --check

build *args="":
    cargo build {{args}}

check:
    cargo check --all-targets

test *args="":
    cargo test {{args}}

clippy *args="":
    cargo clippy --all-targets {{args}}

doc *args="":
    cargo doc --no-deps {{args}}

deny:
    cargo deny check

typos:
    typos
