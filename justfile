name := "cid"

update_dependencies:
    cargo update
    cargo vendor
    cargo sbom | jq '.files = (.files| sort_by(.SPDXID))' | jq '.packages = (.packages| sort_by(.SPDXID))' >{{name}}.sbom.spdx.json
    git add Cargo.lock {{name}}.sbom.spdx.json vendor
    git commit -m "chore(dependencies): bumps versions"
