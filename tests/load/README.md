# Load & Capacity Testing Harness (`k6`)

This directory contains automated stress and capacity verification scripts for the `dry-verify-runner` service.

## Prerequisites

Install [k6](https://k6.io/docs/get-started/installation/):
```bash
# macOS
brew install k6

# Linux (Debian/Ubuntu)
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update && sudo apt-get install k6
```

## Running the Load Test

1. Start the verify runner container locally or in staging:
```bash
docker compose -f deploy/docker-compose.yml up -d
```

2. Run the load test against the service:
```bash
k6 run tests/load/k6-verify.js
```

To target a remote staging instance:
```bash
TARGET_URL="https://verify.staging.dry.run" k6 run tests/load/k6-verify.js
```

## Service Level Objectives (SLOs)

| Metric | Target / Threshold | Description |
|---|---|---|
| **Availability (Uptime)** | $\ge 99.9\%$ | Unplanned downtime $< 43$ minutes/month |
| **p95 Latency** | $< 500\text{ ms}$ | 95th percentile response time for files $< 100\text{k}$ segments |
| **p99 Latency** | $< 1200\text{ ms}$ | 99th percentile response time for files $< 100\text{k}$ segments |
| **Error Rate** | $< 0.01$ ($< 1\%$) | Excludes user-side $422$ input-invalid responses |
| **Peak Memory per Replica** | $< 4\text{ GiB}$ | Memory ceiling enforced by container limits |
