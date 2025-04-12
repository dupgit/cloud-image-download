name := "cid"

bump patch:
    cargo update
    cargo vendor
    cargo release version {{patch}} --no-confirm --execute
    cargo build --release --offline
    cargo test --release
    cargo doc --no-deps
    cargo sbom | jq --sort-keys | jq '.files = (.files| sort_by(.SPDXID))' | jq '.packages = (.packages| sort_by(.SPDXID))' | jq '.relationships = (.relationships| sort_by(.spdxElementId))'>{{name}}.sbom.spdx.json
    git add Cargo.toml Cargo.lock {{name}}.sbom.spdx.json vendor
    cargo release commit --no-confirm --execute
    cargo release tag --no-confirm --execute
