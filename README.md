# Dire Matching Engine

High-performance, deterministic matching engine for financial exchanges: order book, price-time priority matching, REST/WebSocket/FIX APIs, multi-instrument support, and optional persistence.

## Quick start

**Docker (recommended):**

```bash
docker build -t dire-matching-engine .
docker run -p 8080:8080 -p 9876:9876 -e DISABLE_AUTH=true dire-matching-engine
```

- **REST & WebSocket:** http://localhost:8080  
- **FIX:** localhost:9876  

**Cargo:**

```bash
cargo build --release
DISABLE_AUTH=true ./target/release/dire_matching_engine
```

See [project_docs/deployment.md](project_docs/deployment.md) for environment variables (ports, instruments, API keys, persistence).

## Use in your exchange project

You can consume the engine in two ways:

| Mode | Use when | How |
|------|----------|-----|
| **As a service** | You want a standalone matching microservice; your exchange front-end, risk, and traders call it over the network. | Run the binary or Docker image. Configure instruments (`INSTRUMENT_IDS`), auth (`API_KEYS`), and optional persistence (`PERSISTENCE_PATH`). Point your exchange at the REST/WebSocket/FIX endpoints. |
| **As a library** | You want the matching core inside your own process (same binary as your exchange gateway, admin, etc.). | Add as a Cargo dependency. Use `MultiEngine` (or `Engine`) and the public types (`Order`, `Trade`, `ExecutionReport`, etc.). You can use the crate's `api` module to serve HTTP/FIX or wrap the engine in your own server. |

**Packaging checklist** (versioning, Docker tags, optional crates.io): [project_docs/packaging_for_exchange.md](project_docs/packaging_for_exchange.md).

## Documentation

| Topic | Document |
|-------|----------|
| Deployment, env vars, production | [deployment.md](project_docs/deployment.md) |
| Packaging for your exchange | [packaging_for_exchange.md](project_docs/packaging_for_exchange.md) |
| REST, WebSocket, FIX API | [api_documentation.md](project_docs/api_documentation.md) |
| Manual testing (curl) | [manual_testing.md](project_docs/manual_testing.md) |
| API keys and roles | [auth_config.md](project_docs/auth_config.md) |
| Certification suite | [certification_suite.md](project_docs/certification_suite.md) |

## License

MIT OR Apache-2.0
