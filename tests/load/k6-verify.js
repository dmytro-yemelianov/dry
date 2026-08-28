import http from 'k6/http';
import { check, sleep } from 'k6';

// k6 Load Testing Configuration for dry-verify-runner
export const options = {
  stages: [
    { duration: '10s', target: 5 },   // Ramp-up to 5 virtual users
    { duration: '30s', target: 20 },  // Sustained load at 20 VUs
    { duration: '10s', target: 50 },  // Peak burst at 50 VUs
    { duration: '10s', target: 0 },   // Ramp-down
  ],
  thresholds: {
    // SLO: 95% of verification requests must complete within 500ms
    http_req_duration: ['p(95)<500', 'p(99)<1200'],
    // SLO: Error rate must be under 1%
    http_req_failed: ['rate<0.01'],
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';
const REGISTRY_URL = __ENV.REGISTRY_URL || 'http://127.0.0.1:9090';

// Sample G-code payload
const SAMPLE_GCODE = `; Layer 1
G92 E0
G1 Z0.200 F7800.000
G1 X100.000 Y100.000 F7800.000
G1 X110.000 Y100.000 E0.500 F1800.000
G1 X110.000 Y110.000 E1.000 F1800.000
G1 X100.000 Y110.000 E1.500 F1800.000
G1 X100.000 Y100.000 E2.000 F1800.000
; Layer 2
G1 Z0.400 F7800.000
G1 X110.000 Y100.000 E2.500 F1800.000
`;

export default function () {
  // 1. Health check probe
  const healthRes = http.get(`${BASE_URL}/healthz`);
  check(healthRes, {
    'healthz status is 200': (r) => r.status === 200,
    'healthz returns ok:true': (r) => JSON.parse(r.body).ok === true,
  });

  // 2. Metrics endpoint
  const metricsRes = http.get(`${BASE_URL}/metrics`);
  check(metricsRes, {
    'metrics status is 200': (r) => r.status === 200,
    'metrics contains request count': (r) => r.body.includes('dry_verify_requests_total'),
  });

  // 3. Verification request (if registry is available)
  const verifyParams = {
    headers: {
      'Content-Type': 'text/plain',
      'X-Request-ID': `k6-${__VU}-${__ITER}`,
    },
  };

  const verifyUrl = `${BASE_URL}/verify?pack=marlin-pla-i3&version=0.1.0&profile=marlin-pla-i3&registry=${REGISTRY_URL}`;
  const verifyRes = http.post(verifyUrl, SAMPLE_GCODE, verifyParams);

  // Validate response status & headers
  check(verifyRes, {
    'verify response has request ID header': (r) => r.headers['X-Request-Id'] !== undefined,
    'verify response status is 200 or expected stage failure': (r) =>
      r.status === 200 || r.status === 502 || r.status === 422,
  });

  sleep(0.1);
}
