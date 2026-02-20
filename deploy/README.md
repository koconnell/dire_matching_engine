# Deploying to GCP with Kubernetes

Build the container image, push to Google Artifact Registry (or GCR), then deploy to GKE.

## Prerequisites

- Docker (or use Cloud Build)
- `gcloud` CLI
- `kubectl` configured for your GKE cluster

**Reproducible builds:** Run `cargo build` once at the project root and commit `Cargo.lock` so Docker builds use the same dependency versions.

## 1. Build the image

From the **project root** (not `deploy/`):

```bash
docker build -t dire-matching-engine:latest .
```

## 2. Push to Google Artifact Registry (GAR)

Create a repo (once per project):

```bash
# Set your GCP project and region
export GCP_PROJECT=your-project-id
export GCP_REGION=us-central1

gcloud artifacts repositories create dire-matching-engine \
  --repository-format=docker \
  --location=$GCP_REGION \
  --description="Dire matching engine" \
  || true
```

Tag and push:

```bash
export IMAGE=$GCP_REGION-docker.pkg.dev/$GCP_PROJECT/dire-matching-engine/dire-matching-engine:latest

docker tag dire-matching-engine:latest $IMAGE
docker push $IMAGE
```

(If using **Container Registry**, use `gcr.io/$GCP_PROJECT/dire-matching-engine:latest` and `docker push gcr.io/...`.)

## 3. Deploy to GKE

Point the Deployment at your image and apply:

```bash
# Update deployment image (if not using default)
sed -i.bak "s|image:.*|image: $IMAGE|" deploy/kubernetes/deployment.yaml
kubectl apply -f deploy/kubernetes/
```

Or set the image when applying:

```bash
kubectl apply -f deploy/kubernetes/
kubectl set image deployment/dire-matching-engine matching-engine=$IMAGE
kubectl rollout status deployment/dire-matching-engine
```

## 4. Expose (optional)

- **LoadBalancer:** change the Service to `type: LoadBalancer` and run `kubectl get svc` to get the external IP.
- **Ingress:** add an Ingress resource and point it at `dire-matching-engine:80`.

## Endpoints

- **Health (for K8s probes):** `GET /health` → 200 OK
- **Submit order:** `POST /orders` with JSON body (see API below)

## API: POST /orders

Request body (JSON). IDs are numbers; quantity/price are decimal strings or numbers:

```json
{
  "order_id": 1,
  "client_order_id": "c1",
  "instrument_id": 1,
  "side": "Buy",
  "order_type": "Limit",
  "quantity": "10",
  "price": "100",
  "time_in_force": "GTC",
  "timestamp": 1,
  "trader_id": 1
}
```

`instrument_id` must match the engine’s `INSTRUMENT_ID` (default 1). Market orders: omit `price` or set to `null`.

Response 200:

```json
{
  "trades": [...],
  "reports": [...]
}
```

Response 400: `{ "error": "message" }` (e.g. wrong instrument).

## Environment

- `PORT` — default 8080
- `INSTRUMENT_ID` — default 1 (engine only accepts orders for this instrument)
