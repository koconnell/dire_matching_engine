#!/usr/bin/env bash
# Build, push to Google Artifact Registry, and deploy to GKE.
# Usage: ./deploy/deploy-gcp.sh [GCP_PROJECT] [GCP_REGION] [GKE_CLUSTER]
set -e

GCP_PROJECT="${1:-${GCP_PROJECT}}"
GCP_REGION="${2:-${GCP_REGION:-us-central1}}"
GKE_CLUSTER="${3:-${GKE_CLUSTER}}"
REPO_NAME="dire-matching-engine"
IMAGE_NAME="$GCP_REGION-docker.pkg.dev/$GCP_PROJECT/$REPO_NAME/dire-matching-engine:latest"

if [[ -z "$GCP_PROJECT" ]]; then
  echo "Usage: $0 GCP_PROJECT [GCP_REGION] [GKE_CLUSTER]"
  echo "  or set env: GCP_PROJECT, GCP_REGION, GKE_CLUSTER"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

echo "Building Docker image..."
docker build -t "dire-matching-engine:latest" .

echo "Pushing to Artifact Registry..."
gcloud artifacts repositories create "$REPO_NAME" \
  --repository-format=docker \
  --location="$GCP_REGION" \
  --description="Dire matching engine" 2>/dev/null || true

docker tag "dire-matching-engine:latest" "$IMAGE_NAME"
docker push "$IMAGE_NAME"

if [[ -n "$GKE_CLUSTER" ]]; then
  echo "Getting GKE credentials..."
  gcloud container clusters get-credentials "$GKE_CLUSTER" --region="$GCP_REGION" --project="$GCP_PROJECT"

  echo "Applying Kubernetes manifests..."
  kubectl apply -f "$SCRIPT_DIR/kubernetes/"

  echo "Updating deployment image..."
  kubectl set image deployment/dire-matching-engine matching-engine="$IMAGE_NAME" --record
  kubectl rollout status deployment/dire-matching-engine --timeout=120s

  echo "Done. Service: dire-matching-engine (ClusterIP). Get endpoints: kubectl get svc dire-matching-engine"
else
  echo "Skipping deploy (no GKE_CLUSTER). Image: $IMAGE_NAME"
  echo "To deploy: kubectl set image deployment/dire-matching-engine matching-engine=$IMAGE_NAME"
fi
