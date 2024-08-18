ci: fmt clippy test audit typos

fmt:
  cargo fmt --check

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
