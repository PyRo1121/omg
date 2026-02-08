# OMG Workers API Tests

## Overview

This directory contains comprehensive API tests for the OMG Cloudflare Workers endpoints, focusing on telemetry and privacy compliance (GDPR/CCPA).

## Test Structure

```
tests/
├── setup.sql              # Database schema for testing
├── test-utils.ts          # Helper functions for test setup/teardown
├── telemetry.test.ts      # Telemetry endpoint tests
├── privacy.test.ts        # Privacy/GDPR endpoint tests
└── README.md              # This file
```

## Running Tests

```bash
# Run all tests
npm test

# Run tests in watch mode
npm run test:watch

# Run specific test file
npm test telemetry.test.ts

# Run tests with verbose output
npm test -- --reporter=verbose
```

## Test Coverage

### Telemetry API (`/api/cli/event`, `/api/cli/batch`)

#### Single Event Tests
- ✅ Accept and store valid command events
- ✅ Store session events
- ✅ Store performance metric events
- ✅ Store feature usage events
- ✅ Return 401 when license_key is missing
- ✅ Return 401 when license_key is invalid
- ✅ Return 401 when license is inactive
- ✅ Return 400 for malformed events
- ✅ Return 500 for malformed JSON

#### Batch Event Tests
- ✅ Process batches of mixed event types atomically
- ✅ Return success with 0 processed for empty batch
- ✅ Return 401 when license_key is missing
- ✅ Return 401 when license_key is invalid
- ✅ Handle large batches (100+ events)

### Privacy API (GDPR/CCPA Compliance)

#### Privacy Status (`GET /api/privacy/status`)
- ✅ Return privacy policy without license key
- ✅ Return user status with valid license key
- ✅ Return null user_status for invalid license key
- ✅ Show available rights (access, deletion, opt-out, portability)

#### Data Export (`POST /api/privacy/export`)
- ✅ Export all user data with valid license key
- ✅ Export data with email instead of license
- ✅ Return 400 without email or license_key
- ✅ Return 404 for non-existent user
- ✅ Create audit log entry for exports
- ✅ Include proper export format (JSON, structured)

#### Data Deletion (`POST /api/privacy/delete`)
- ✅ Require `confirm: true` flag
- ✅ Delete all user data (telemetry, usage, notes)
- ✅ Delete by license_key, email, or machine_id
- ✅ Anonymize license record (soft delete)
- ✅ Create audit log entry for deletions
- ✅ Return 400 without identifier
- ✅ Return 404 for non-existent user
- ✅ Preserve payment records (Stripe requirement)

#### Telemetry Opt-Out (`POST /api/privacy/opt-out`)
- ✅ Opt-out of telemetry collection
- ✅ Opt-in to telemetry (re-enable)
- ✅ Return 400 without license_key
- ✅ Return 404 for invalid license_key
- ✅ Keep license functional after opt-out

## Test Data Setup

All tests use isolated in-memory D1 databases via Miniflare. Each test suite:

1. **beforeEach**: Creates fresh test customer, license, and telemetry data
2. **test**: Executes test scenarios
3. **afterEach**: Cleans up all test data

### Test Customer Structure

```typescript
{
  customerId: 'test-customer-xyz',
  licenseId: 'test-license-xyz',
  licenseKey: 'test-key-xyz',
  email: 'test@example.com',
  tier: 'pro'
}
```

## Database Schema

See `setup.sql` for the complete test schema. Key tables:

- `customers` - Customer records
- `licenses` - License keys
- `command_event` - CLI command telemetry
- `session` - User session tracking
- `performance_metric` - Performance metrics
- `feature_usage` - Feature adoption tracking
- `audit_log` - Audit trail for privacy actions

## Helper Functions

### Database Setup
```typescript
setupDatabase(db: D1Database)         // Initialize schema
clearAllTables(db: D1Database)        // Clean all data
```

### Test Customer Management
```typescript
createTestCustomer(db, email, tier)  // Create test customer with license
deleteTestCustomer(db, customerId)   // Delete customer and cascade
```

### Test Data Creation
```typescript
createTelemetryData(db, licenseId, machineId)  // Seed telemetry data
```

## Assertions

All tests use Vitest assertions:

```typescript
expect(response.status).toBe(200)
expect(body).toHaveProperty('success', true)
expect(body.error).toContain('License key required')
```

## CI/CD Integration

These tests run on:
- Every pull request
- Pre-deployment checks
- Scheduled nightly runs

### GitHub Actions Workflow

```yaml
- name: Run API Tests
  run: |
    cd site/workers
    npm install
    npm test
```

## Privacy Compliance Notes

The privacy tests ensure **global availability** of GDPR/CCPA rights:

- ✅ Right to Access (data export)
- ✅ Right to Deletion (forget me)
- ✅ Right to Opt-Out (telemetry)
- ✅ Right to Portability (JSON export)

These rights are available to **ALL users worldwide**, not just EU/California residents.

## Performance Benchmarks

Test execution targets:
- Single test: < 100ms
- Full suite: < 5s
- Database setup: < 500ms

## Debugging

### View Test Output
```bash
npm test -- --reporter=verbose
```

### Debug Single Test
```bash
npm test -- --grep "should accept and store a valid command event"
```

### Database State Inspection
Add this to any test:
```typescript
const data = await env.DB.prepare('SELECT * FROM command_event').all();
console.log(data);
```

## Related Documentation

- [Cloudflare Workers Testing](https://developers.cloudflare.com/workers/testing/)
- [Vitest Documentation](https://vitest.dev/)
- [D1 Database API](https://developers.cloudflare.com/d1/api/)
- [GDPR Compliance Guide](https://gdpr.eu/)

## Contributing

When adding new tests:

1. Follow existing test structure
2. Use descriptive test names
3. Clean up test data in `afterEach`
4. Add test coverage to this README
5. Ensure tests are idempotent
