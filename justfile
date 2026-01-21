fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt --all
    cargo sort --workspace --grouped