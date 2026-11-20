# Releasing

Releases are published by the [release workflow](../../.github/workflows/release.yml) when a version tag is pushed. It implements [ADR-0012](../adr/0012-distribution.md).

## Steps

1. Set the same version in `backend/Cargo.toml` and `frontend/package.json`, then refresh the lockfile and the API spec:

   ```sh
   cd backend
   cargo update -p soabox
   cargo run --quiet -- openapi > openapi.json
   ```

2. Run the checks in [Testing](testing.md) and merge the change into `main`.
3. Push `main`, then tag the release commit and push the tag:

   ```sh
   git tag -a v0.2.1 -m "SOAbox v0.2.1"
   git push origin v0.2.1
   ```

## What The Workflow Does

1. Runs the full CI suite on the tagged commit.
2. Fails if the tag does not match the backend and frontend versions.
3. Builds the image and checks that it runs as UID 1001 and reports the tagged version.
4. Scans the image with Trivy and fails on fixable High or Critical findings.
5. Publishes to `ghcr.io/invite-networks/soabox` with these tags:

   | Tag | Example | Moves when |
   |---|---|---|
   | Full version | `0.2.1` | Never. One release, one tag. |
   | Minor line | `0.2` | Each patch release in that line. |
   | `latest` | `latest` | Each release that is not a prerelease. |

   A prerelease tag such as `v1.0.0-rc.1` publishes only `1.0.0-rc.1`.

6. Attaches a signed build provenance attestation to the image digest. Verify it with:

   ```sh
   gh attestation verify oci://ghcr.io/invite-networks/soabox:0.2.1 --owner invite-networks
   ```

The workflow summary lists the published tags and the image digest.

## Failed Releases

If the workflow fails, nothing was published. Fix the problem on `main`, delete the tag locally and on GitHub, and tag the fixed commit again. A version that was already published is never overwritten. Release the fix as the next patch version instead.
