import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const transactionLatency = new Trend('transaction_latency');

export const options = {
  stages: [
    { duration: '30s', target: 50 },   // Ramp up to 50 users
    { duration: '60s', target: 200 },  // Stay at 200 users for 60 seconds (200 TPS requirement)
    { duration: '30s', target: 0 },    // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'], // p95 latency < 200ms requirement
    errors: ['rate<0.01'],            // Error rate < 1%
    transaction_latency: ['p(95)<200'],
  },
};

const BASE_URL = 'http://localhost:3001';

export default function () {
  // Test transaction creation
  const payload = {
    external_id: `test-${__VU}-${__ITER}-${Date.now()}`,
    amount: Math.floor(Math.random() * 10000) + 100, // Random amount between 100-10100 cents
    currency: 'USD',
    from_account: `account-${Math.floor(Math.random() * 1000)}`,
    to_account: `account-${Math.floor(Math.random() * 1000)}`,
    description: `Test transaction ${__VU}-${__ITER}`,
    metadata: {
      test_run: 'load_test',
      virtual_user: __VU.toString(),
      iteration: __ITER.toString(),
    }
  };

  const headers = {
    'Content-Type': 'application/json',
  };

  const startTime = Date.now();
  const response = http.post(`${BASE_URL}/transactions`, JSON.stringify(payload), { headers });
  const endTime = Date.now();

  // Record custom metrics
  transactionLatency.add(endTime - startTime);
  errorRate.add(response.status !== 200);

  // Verify response
  const success = check(response, {
    'status is 200': (r) => r.status === 200,
    'response has transaction id': (r) => {
      try {
        const data = JSON.parse(r.body);
        return data.success && data.data && data.data.id;
      } catch (e) {
        return false;
      }
    },
    'response time < 200ms': (r) => r.timings.duration < 200,
  });

  if (!success) {
    console.error(`Request failed: ${response.status} - ${response.body}`);
  }

  // Test idempotency with same external_id
  if (Math.random() < 0.1) { // 10% chance to test idempotency
    const duplicateResponse = http.post(`${BASE_URL}/transactions`, JSON.stringify(payload), { headers });
    check(duplicateResponse, {
      'duplicate request handled correctly': (r) => r.status === 400 || r.status === 409,
    });
  }

  // Randomly test transaction state changes
  if (Math.random() < 0.3 && response.status === 200) { // 30% chance
    try {
      const data = JSON.parse(response.body);
      if (data.success && data.data && data.data.id) {
        const transactionId = data.data.id;
        
        // Randomly commit or fail the transaction
        if (Math.random() < 0.8) { // 80% commit
          const commitResponse = http.post(`${BASE_URL}/transactions/${transactionId}/commit`, null, { headers });
          check(commitResponse, {
            'commit successful': (r) => r.status === 200,
          });
        } else { // 20% fail
          const failResponse = http.post(`${BASE_URL}/transactions/${transactionId}/fail`, null, { headers });
          check(failResponse, {
            'fail successful': (r) => r.status === 200,
          });
        }
      }
    } catch (e) {
      console.error('Error parsing response for state change test:', e);
    }
  }

  sleep(0.1); // Small delay between requests
}

export function handleSummary(data) {
  return {
    'load-test-results.json': JSON.stringify(data, null, 2),
    stdout: `
Load Test Results:
==================
Duration: ${data.metrics.iteration_duration.values.avg.toFixed(2)}ms
Requests: ${data.metrics.http_reqs.values.count}
Failed Requests: ${data.metrics.http_req_failed.values.count}
Error Rate: ${(data.metrics.http_req_failed.values.rate * 100).toFixed(2)}%
P95 Latency: ${data.metrics.http_req_duration.values['p(95)'].toFixed(2)}ms
Max Latency: ${data.metrics.http_req_duration.values.max.toFixed(2)}ms
Avg Latency: ${data.metrics.http_req_duration.values.avg.toFixed(2)}ms
Throughput: ${data.metrics.http_reqs.values.rate.toFixed(2)} req/s

Requirements Check:
===================
✅ P95 Latency < 200ms: ${data.metrics.http_req_duration.values['p(95)'] < 200 ? 'PASS' : 'FAIL'} (${data.metrics.http_req_duration.values['p(95)'].toFixed(2)}ms)
✅ Error Rate < 1%: ${data.metrics.http_req_failed.values.rate < 0.01 ? 'PASS' : 'FAIL'} (${(data.metrics.http_req_failed.values.rate * 100).toFixed(2)}%)
✅ Target Throughput: ${data.metrics.http_reqs.values.rate >= 200 ? 'PASS' : 'FAIL'} (${data.metrics.http_reqs.values.rate.toFixed(2)} req/s)
    `,
  };
}





