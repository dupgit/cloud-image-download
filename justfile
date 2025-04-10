name := "cid"

update_dependencies:
    cargo update
    cargo sbom | jq '.files = (.files| sort_by(.SPDXID))' | jq '.packages = (.packages| sort_by(.SPDXID))' >{{name}}.sbom.spdx.json
    git add Cargo.lock {{name}}.sbom.spdx.json
    git commit -m "chore(dependencies): bumps versions"
