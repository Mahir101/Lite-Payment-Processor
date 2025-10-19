import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics for reconciliation service
const errorRate = new Rate('reconciliation_errors');
const reportLatency = new Trend('report_latency');

export const options = {
  stages: [
    { duration: '10s', target: 10 },   // Ramp up
    { duration: '30s', target: 50 },   // Stay at 50 users
    { duration: '10s', target: 0 },    // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // More lenient for reconciliation
    errors: ['rate<0.05'],            // Error rate < 5%
    report_latency: ['p(95)<500'],
  },
};

const BASE_URL = 'http://localhost:3002';

export default function () {
  // Test health check
  const healthResponse = http.get(`${BASE_URL}/health`);
  check(healthResponse, {
    'health check successful': (r) => r.status === 200,
  });

  // Test listing reports
  const reportsResponse = http.get(`${BASE_URL}/reports?limit=10`);
  check(reportsResponse, {
    'reports list successful': (r) => r.status === 200,
  });

  // Test listing anomalies
  const anomaliesResponse = http.get(`${BASE_URL}/anomalies?limit=10`);
  check(anomaliesResponse, {
    'anomalies list successful': (r) => r.status === 200,
  });

  // Test daily summaries
  const summariesResponse = http.get(`${BASE_URL}/daily-summaries?limit=7`);
  check(summariesResponse, {
    'daily summaries successful': (r) => r.status === 200,
  });

  // Test report generation (occasionally)
  if (Math.random() < 0.1) { // 10% chance
    const now = new Date();
    const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000);
    
    const payload = {
      period_start: yesterday.toISOString(),
      period_end: now.toISOString(),
    };

    const startTime = Date.now();
    const generateResponse = http.post(
      `${BASE_URL}/reports/generate`,
      JSON.stringify(payload),
      { headers: { 'Content-Type': 'application/json' } }
    );
    const endTime = Date.now();

    reportLatency.add(endTime - startTime);
    errorRate.add(generateResponse.status !== 200);

    check(generateResponse, {
      'report generation successful': (r) => r.status === 200,
    });
  }

  // Test reconciliation trigger (rarely)
  if (Math.random() < 0.05) { // 5% chance
    const reconcileResponse = http.post(`${BASE_URL}/reconcile`);
    check(reconcileResponse, {
      'reconciliation trigger successful': (r) => r.status === 200,
    });
  }

  sleep(1); // Longer delay for reconciliation service
}

export function handleSummary(data) {
  return {
    'reconciliation-test-results.json': JSON.stringify(data, null, 2),
    stdout: `
Reconciliation Service Test Results:
===================================
Duration: ${data.metrics.iteration_duration.values.avg.toFixed(2)}ms
Requests: ${data.metrics.http_reqs.values.count}
Failed Requests: ${data.metrics.http_req_failed.values.count}
Error Rate: ${(data.metrics.http_req_failed.values.rate * 100).toFixed(2)}%
P95 Latency: ${data.metrics.http_req_duration.values['p(95)'].toFixed(2)}ms
Max Latency: ${data.metrics.http_req_duration.values.max.toFixed(2)}ms
Avg Latency: ${data.metrics.http_req_duration.values.avg.toFixed(2)}ms
Throughput: ${data.metrics.http_reqs.values.rate.toFixed(2)} req/s
    `,
  };
}





