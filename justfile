fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt --all
    # cargo shear --fix exits 1 even when it fixes everything, so re-check to get the real result.
    cargo shear --fix || cargo shear
    cargo sort --workspace --grouped