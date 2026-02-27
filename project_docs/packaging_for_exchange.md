# Packaging for your financial exchange project

This document describes how to version, build, and ship the Dire Matching Engine so it’s ready to plug into your exchange (Docker, registry, optional crates.io).

---

## How you can use it

- **As a service (recommended for most exchanges)**  
  Run the engine as a separate process or container. Your exchange front-end, risk engine, and traders call it via REST, WebSocket, or FIX. You get a single, versioned binary or image to deploy.

- **As a library**  
  Add the crate as a Cargo dependency and embed `MultiEngine` (or `Engine`) in your own binary. Use the public API (`Order`, `Trade`, `ExecutionReport`, `MatchingEngine`, etc.) and optionally the crate’s `api` module for HTTP/FIX. Use this when the engine should live in the same process as your gateway or admin.

---

## Checklist: ready for the exchange

1. **Version**  
   Set a stable version in `Cargo.toml` (e.g. `version = "0.2.0"` or `"1.0.0"`). Bump when you cut a release.

2. **Reproducible builds**  
   Commit `Cargo.lock` so Docker and CI build the same dependency tree. (If `Cargo.lock` is in `.gitignore`, remove it from `.gitignore` and add the file.)

3. **Docker image**  
   - Build: `docker build -t dire-matching-engine:<version> .`  
   - Tag with a version (e.g. `0.2.0`) and push to your registry (e.g. GitHub Container Registry, GCP Artifact Registry, ECR).  
   - In your exchange’s deploy pipeline, pull `dire-matching-engine:<version>` and run with the right env (see [deployment.md](deployment.md)).

4. **Environment in production**  
   - Set `API_KEYS` (and do not set `DISABLE_AUTH`) for production.  
   - Use `INSTRUMENT_IDS` (and optionally `PERSISTENCE_PATH` with a mounted volume).  
   - Expose ports 8080 (REST/WebSocket) and 9876 (FIX) or map them in your ingress/load balancer.

5. **(Optional) Publish to crates.io**  
   If you want to depend on the engine from another Rust project via crates.io:

   ```bash
   cargo publish
   ```

   Then in your exchange workspace:

   ```toml
   [dependencies]
   dire_matching_engine = "0.2"
   ```

   If you keep the engine in a monorepo or private repo, use a path or git dependency instead:

   ```toml
   dire_matching_engine = { path = "../dire_matching_engine" }
   # or
   dire_matching_engine = { git = "https://github.com/yourorg/dire_matching_engine", tag = "v0.2.0" }
   ```

---

## Example: Docker with version and persistence

```bash
# Build
docker build -t dire-matching-engine:0.2.0 .

# Run with persistence and API keys (mount volume for state file)
docker run -p 8080:8080 -p 9876:9876 \
  -e API_KEYS="trader-key:trader,admin-key:admin,ops-key:operator" \
  -e INSTRUMENT_IDS="1:AAPL,2:GOOG" \
  -e PERSISTENCE_PATH=/data/state.json \
  -v engine-data:/data \
  dire-matching-engine:0.2.0
```

---

## Example: Git tag release

```bash
git tag -a v0.2.0 -m "Release 0.2.0"
git push origin v0.2.0
```

Then build and push your image using that tag so your exchange always deploys a known version.

---

## See also

- [deployment.md](deployment.md) — Ports, env vars, production notes  
- [api_documentation.md](api_documentation.md) — REST, WebSocket, FIX  
- [deploy/README.md](../deploy/README.md) — GCP/Kubernetes deploy
