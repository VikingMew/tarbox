# Tarbox CSI Driver Helm Chart

This Helm chart deploys the Tarbox CSI Driver for Kubernetes, enabling PostgreSQL-based persistent storage with multi-tenancy, layering, and snapshotting capabilities.

## Prerequisites

- Kubernetes 1.20+
- Helm 3.0+
- PostgreSQL 14+ database accessible from the cluster
- CSI snapshot CRDs installed (for snapshot support)

## Installation

### 1. Create Database Secret

```bash
kubectl create secret generic tarbox-db-credentials \
  --from-literal=url='postgresql://user:password@hostname:5432/tarbox' \
  -n kube-system
```

### 2. Install the Chart

```bash
helm install tarbox-csi ./charts/tarbox-csi \
  --namespace kube-system \
  --create-namespace
```

### 3. Verify Installation

```bash
# Check controller pods
kubectl get pods -n kube-system -l app=tarbox-csi-controller

# Check node pods
kubectl get pods -n kube-system -l app=tarbox-csi-node

# Check CSI driver registration
kubectl get csidrivers tarbox.csi.io
```

## Configuration

The following table lists the configurable parameters and their default values.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.repository` | Tarbox CSI image repository | `tarbox/csi-driver` |
| `image.tag` | Image tag | `latest` |
| `database.url` | PostgreSQL connection URL | `postgresql://...` |
| `database.existingSecret` | Existing secret name for DB credentials | `""` |
| `controller.replicas` | Number of controller replicas | `2` |
| `storageClass.create` | Create default StorageClass | `true` |
| `volumeSnapshotClass.create` | Create VolumeSnapshotClass | `true` |
| `metrics.enabled` | Enable Prometheus metrics | `true` |

See `values.yaml` for full configuration options.

## Usage Examples

### Create a PersistentVolumeClaim

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: my-tarbox-volume
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
```

### Create a VolumeSnapshot

```yaml
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: my-snapshot
spec:
  volumeSnapshotClassName: tarbox-snapshot
  source:
    persistentVolumeClaimName: my-tarbox-volume
```

### Use Volume in a Pod

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-app
spec:
  containers:
    - name: app
      image: myapp:latest
      volumeMounts:
        - name: data
          mountPath: /data
  volumes:
    - name: data
      persistentVolumeClaim:
        claimName: my-tarbox-volume
```

## Uninstallation

```bash
helm uninstall tarbox-csi -n kube-system
```

## Troubleshooting

### Check Controller Logs

```bash
kubectl logs -n kube-system -l app=tarbox-csi-controller -c tarbox-csi
```

### Check Node Logs

```bash
kubectl logs -n kube-system -l app=tarbox-csi-node -c tarbox-csi
```

### Verify Database Connection

```bash
kubectl exec -n kube-system deployment/tarbox-csi-controller -c tarbox-csi -- \
  psql $DATABASE_URL -c "SELECT 1"
```

## License

See the main Tarbox repository for license information.
