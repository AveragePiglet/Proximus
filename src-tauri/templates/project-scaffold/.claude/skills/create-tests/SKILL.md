---
name: create-tests
description: Write effective Vitest integration tests for any module or feature — server actions, API route handlers, utilities, or business logic. Use this skill whenever the user runs `/create-tests`, says "write tests for this", "add tests", "add test coverage", or points at a specific file/feature and asks for tests. The skill discovers the project's test setup, analyses the target code's external boundaries, designs a complete test plan, then writes working test files that pass on first run. Produces integration-style tests that mock external I/O (DB, APIs, email, queues) while exercising real application logic — not shallow unit tests that just verify mock calls, and not fragile e2e tests that need a live database.
---

# Create Vitest Integration Tests

Analyse the target code, design a complete test plan, and write Vitest integration tests that pass on first run. Tests mock external I/O boundaries while exercising real application logic — the best cost-to-value ratio for server-side code.

---

## When this skill runs

The user has typed `/create-tests` or asked you to write tests for a specific module, file, or feature. Your job is to understand what the code does, identify its external boundaries, design meaningful tests, and write them — not ask the user to write them.

---

## Step 1 — Clarify scope (only if genuinely ambiguous)

If the user named a specific file or feature, proceed directly. Only ask if it's truly unclear what to test.

Typical scope patterns:
- `/create-tests for src/app/api/orders/route.ts` → test that specific handler
- `/create-tests for the checkout flow` → find and test the server action(s) that back it
- `/create-tests` (no args, uncommitted changes visible) → test whatever was just changed

---

## Step 2 — Discover the project's test setup

Before writing a single line, understand how the project runs tests. Run these in order:

```bash
# 1. Check for an existing test config
cat vitest.config.ts 2>/dev/null || cat vitest.config.js 2>/dev/null || cat jest.config.ts 2>/dev/null || echo "NO CONFIG FOUND"

# 2. Check package.json for test scripts and test-related deps
cat package.json | grep -E '"test|vitest|jest|@testing-library|coverage'

# 3. Find existing test files to learn conventions already established
find . -name "*.test.ts" -o -name "*.test.tsx" -o -name "*.spec.ts" | grep -v node_modules | head -20

# 4. Check tsconfig for path aliases (@/ etc.)
cat tsconfig.json | grep -A5 '"paths"'
```

If no test framework is installed yet, set up Vitest before writing any tests:
- Install: `npm install -D vitest @vitest/coverage-v8`
- If the project uses TypeScript path aliases (`@/`): `npm install -D vite-tsconfig-paths`
- Create `vitest.config.ts` (see the Configuration Reference section below)
- Add `"test": "vitest run"`, `"test:watch": "vitest"`, `"test:coverage": "vitest run --coverage"` to `package.json` scripts

---

## Step 3 — Read and analyse the target code

Read every file that is in scope. For each file, identify:

**External boundaries** — everything that must be mocked:
- Database clients (Drizzle `db`, Prisma client, Mongoose, Sequelize, raw `pg`)
- HTTP clients (fetch, axios, Stripe SDK, Resend, Twilio, SendGrid, AWS SDK)
- File system (`fs`, S3 clients, presigned URL generators)
- Authentication helpers (`getSession`, `auth()`, `verifyToken`)
- Cache/queue clients (Redis, BullMQ, SQS)
- Third-party service clients (Stripe, Braintree, PayPal, Algolia, etc.)
- Environment-dependent modules that throw on missing env vars at import time

**Internal logic to actually test** — this is what matters:
- Input validation (Zod schemas, manual checks, type coercions)
- Business rules (price calculations, capacity checks, status transitions)
- Branching conditions (what happens when X is missing / invalid / expired)
- Error handling paths (what gets returned / thrown / logged for each failure mode)
- Security guards (auth checks, ownership checks, tenant isolation, redirect validation)
- Side effects triggered in the right conditions (emails enqueued, sessions expired, DB rows updated)

**Critical Vitest constraint to check before writing any mocks:**

Vitest **hoists** `vi.mock()` calls to the top of the file before any imports are evaluated. This means:

- ✅ **Works**: Define `vi.fn()` spies at module scope in the test file, then reference them inside the `vi.mock()` factory (the factory is a closure over the module scope)
- ✅ **Works**: Use `vi.mock('module', () => ({ ... }))` with the factory returning a plain object (no external references needed)
- ❌ **Breaks**: `require('./helpers/mocks')` inside a `vi.mock()` factory — the relative path can't resolve when hoisted
- ❌ **Breaks**: Referencing a `const spy = vi.fn()` declared with `const` inside the factory — hoisted code runs before `const` declarations

**The correct pattern:**
```typescript
// ✅ Module-scope spies — defined before vi.mock() hoisting runs
const mockDbInsert = vi.fn()
const mockStripeCreate = vi.fn()

vi.mock('@/lib/db', () => ({
  db: {
    insert: (...args: unknown[]) => mockDbInsert(...args),
    // ...
  },
}))

vi.mock('@/lib/stripe', () => ({
  stripe: {
    checkout: { sessions: { create: (...args: unknown[]) => mockStripeCreate(...args) } },
  },
}))
```

---

## Step 4 — Design the test plan

Before writing code, mentally lay out the full test suite. A good integration test suite covers:

### Coverage checklist

**Happy path (at least one)**
- [ ] The function returns the correct success shape with valid inputs
- [ ] The right external calls were made (DB insert, API call, email enqueued)
- [ ] The right data was passed to those calls (correct fields, correct values)

**Input validation** (for every required/validated field)
- [ ] Rejects null / undefined input entirely
- [ ] Rejects empty strings on required fields
- [ ] Rejects invalid formats (bad email, non-UUID, negative number, etc.)
- [ ] Rejects values that are too long / too large / out of range
- [ ] Returns error messages that point at the right field

**Guard clauses** (for every early-return condition in the code)
- [ ] Each "not found" path returns the right error without calling downstream services
- [ ] Each "forbidden" path (wrong owner, wrong tenant, wrong status) is tested
- [ ] Each "already done" idempotency path (duplicate, already confirmed, etc.) is tested

**Business rules**
- [ ] Price / quantity / capacity calculations are tested with specific numbers
- [ ] Status transitions only happen from the right current state
- [ ] Concurrent-access guards (atomic DB WHERE clauses) are exercised

**Error recovery**
- [ ] When an external call throws, the function returns a clean error (not an unhandled rejection)
- [ ] Cleanup side effects happen correctly on failure (sessions expired, partial writes rolled back)

**Security** (where applicable)
- [ ] Tenant / ownership isolation — cross-tenant access is rejected
- [ ] Open redirect prevention — external URLs are rejected
- [ ] Signature / token verification failures return the right status

---

## Step 5 — Write the test files

### File placement

Follow the project convention found in Step 2. Common patterns:
- `src/tests/feature-name.test.ts` (dedicated test directory)
- `src/app/api/orders/__tests__/route.test.ts` (co-located with source)
- `src/__tests__/feature-name.test.ts` (root tests directory)

If no convention exists, use `src/tests/` as the default.

### File structure template

```typescript
/**
 * Integration tests for src/path/to/module.ts
 *
 * Strategy: mock all external I/O boundaries (DB, third-party APIs, auth).
 * Tests exercise the real application logic — validation, business rules,
 * error handling — without touching a real database or making real API calls.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'

// ─── Env vars before module load ──────────────────────────────────────────────
// Modules that throw on missing env vars must have these set before vi.mock()
// resolves their imports. vi.stubEnv() is the correct Vitest API for this.

vi.stubEnv('DATABASE_URL', 'postgres://test:test@localhost:5432/test')
vi.stubEnv('STRIPE_SECRET_KEY', 'sk_test_placeholder')
// ... add any other env vars the module reads at import time

// ─── Module-scope spies ───────────────────────────────────────────────────────
// Must be at module scope so vi.mock() factories below can close over them.
// DO NOT declare these inside the factory — vi.mock() is hoisted and the
// factory runs before const declarations in the test body.

const mockDbInsert = vi.fn()
const mockDbSelect = vi.fn()
const mockDbUpdate = vi.fn()
const mockDbDelete = vi.fn()
const mockDbTransaction = vi.fn()
// ... add one spy per distinct external method you need to control

// ─── Mock external modules ────────────────────────────────────────────────────
// Each vi.mock() factory returns the shape the module under test imports.
// The factory closes over the module-scope spies defined above.

vi.mock('@/lib/db', () => ({
  db: {
    insert: (...args: unknown[]) => mockDbInsert(...args),
    select: (...args: unknown[]) => mockDbSelect(...args),
    update: (...args: unknown[]) => mockDbUpdate(...args),
    delete: (...args: unknown[]) => mockDbDelete(...args),
    transaction: (...args: unknown[]) => mockDbTransaction(...args),
  },
  // Re-export schema symbols as plain Symbols — they're passed to mocked
  // Drizzle methods as opaque references; the methods don't use them.
  users: Symbol('users'),
  orders: Symbol('orders'),
  // ... match the named exports the module imports from this path
}))

vi.mock('@/lib/stripe', () => ({
  stripe: {
    checkout: {
      sessions: {
        create: (...args: unknown[]) => mockStripeCreate(...args),
      },
    },
  },
}))

// ─── Import the module under test ─────────────────────────────────────────────
// Imports happen after vi.mock() is hoisted, so all mocks are in place.

import { myFunction } from '@/path/to/module'
import { mockFixture } from './helpers/fixtures' // if you have shared fixtures

// ─── Test helpers ─────────────────────────────────────────────────────────────

/** Returns a valid happy-path input, merging in any per-test overrides. */
function validInput(overrides: Record<string, unknown> = {}) {
  return { ...DEFAULT_INPUT, ...overrides }
}

/**
 * Set up the DB mock chain for a successful query.
 * Drizzle uses a fluent builder pattern — each method returns the builder.
 * The terminal operation (returning, execute, or just awaiting) resolves the promise.
 */
function setupSuccessfulDb() {
  mockDbInsert.mockImplementation(() => {
    const returning = vi.fn().mockResolvedValue([{ id: 'abc-123', ...OTHER_FIELDS }])
    return { values: vi.fn().mockReturnValue({ returning }) }
  })

  mockDbTransaction.mockImplementation(async (cb: (tx: unknown) => Promise<void>) => {
    // Build a minimal tx object with the same mock methods
    const tx = {
      insert: mockDbInsert,
      select: mockDbSelect,
      update: mockDbUpdate,
      delete: mockDbDelete,
    }
    return cb(tx)
  })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe('myFunction', () => {

  beforeEach(() => {
    // Reset to happy-path defaults before each test.
    // Tests that need different behaviour override these inline.
    setupSuccessfulDb()
    mockStripeCreate.mockResolvedValue({ id: 'cs_test', url: 'https://stripe.com/pay/cs_test', amount_total: 1000 })
  })

  describe('happy path', () => {
    it('returns the expected success shape', async () => {
      const result = await myFunction(validInput())
      expect(result.success).toBe(true)
      if (!result.success) throw new Error('narrowing') // type-narrowing guard
      expect(result.data).toMatchObject({ /* key fields */ })
    })

    it('calls the DB with the correct values', async () => {
      await myFunction(validInput())
      expect(mockDbInsert).toHaveBeenCalledTimes(1)
      // Assert on what was passed, not just that it was called
    })
  })

  describe('input validation', () => {
    it('rejects null input', async () => {
      const result = await myFunction(null)
      expect(result.success).toBe(false)
    })

    it('rejects missing required field X', async () => {
      const result = await myFunction(validInput({ fieldX: '' }))
      expect(result.success).toBe(false)
      if (result.success) throw new Error('narrowing')
      expect(result.error).toMatch(/fieldX/i)
    })
  })

  // ... more describe blocks per logical area
})
```

### Mocking Drizzle ORM query chains

Drizzle uses a fluent builder — every method returns `this`, and the final method returns a Promise. Mock it like this:

```typescript
// Pattern: select().from().where().limit()
mockDbSelect.mockImplementation(() => {
  const limit = vi.fn().mockResolvedValue([rowFixture])  // ← resolves the chain
  const where = vi.fn().mockReturnValue({ limit })
  return { from: vi.fn().mockReturnValue({ where }) }
})

// Pattern: select().from().where()  (no .limit() — awaited directly)
mockDbSelect.mockImplementation(() => {
  const where = vi.fn().mockResolvedValue([rowFixture])  // ← where() resolves
  return { from: vi.fn().mockReturnValue({ where }) }
})

// Pattern: update().set().where().returning()
mockDbUpdate.mockImplementation(() => {
  const returning = vi.fn().mockResolvedValue([updatedRow])
  const where = vi.fn().mockReturnValue({ returning })
  const set = vi.fn().mockReturnValue({ where })
  return { set }
})

// Pattern: insert().values().returning()
mockDbInsert.mockImplementation(() => {
  const returning = vi.fn().mockResolvedValue([insertedRow])
  const values = vi.fn().mockReturnValue({ returning })
  return { values }
})

// Pattern: delete().where()
mockDbDelete.mockImplementation(() => {
  const where = vi.fn().mockResolvedValue([])
  return { where }
})
```

**Key rule**: The method that terminates the chain (the one actually `await`ed) must return a **Promise** (e.g. `vi.fn().mockResolvedValue(...)` not `vi.fn().mockReturnValue(...)`). Every intermediate method must return a plain object (not a Promise).

### Mocking transactions

```typescript
mockDbTransaction.mockImplementation(async (cb: (tx: unknown) => Promise<void>) => {
  let insertCallCount = 0
  const tx = {
    insert: vi.fn().mockImplementation(() => {
      insertCallCount++
      if (insertCallCount === 1) {
        // First insert: bookings
        const returning = vi.fn().mockResolvedValue([{ id: 'booking-id', reference: 'REF-001' }])
        return { values: vi.fn().mockReturnValue({ returning }) }
      }
      // Second insert: line items, players, etc.
      return { values: vi.fn().mockReturnValue({ returning: vi.fn().mockResolvedValue([]) }) }
    }),
    update: vi.fn().mockImplementation(() => ({
      set: vi.fn().mockReturnValue({
        where: vi.fn().mockReturnValue({
          returning: vi.fn().mockResolvedValue([]),
        }),
      }),
    })),
    select: vi.fn().mockImplementation(() => ({
      from: vi.fn().mockReturnValue({
        where: vi.fn().mockReturnValue({
          limit: vi.fn().mockResolvedValue([]),
        }),
      }),
    })),
    delete: vi.fn().mockReturnValue({ where: vi.fn().mockResolvedValue([]) }),
  }
  return cb(tx) // propagates throws — AlreadyExistsError etc. bubble up correctly
})
```

### Mocking Next.js API route handlers

For `export async function POST(req: NextRequest)` style handlers:

```typescript
import { NextRequest } from 'next/server'

function makeRequest(body: object, headers: Record<string, string> = {}) {
  return new NextRequest('http://localhost:3000/api/your-route', {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...headers },
    body: JSON.stringify(body),
  })
}

it('returns 200 on valid request', async () => {
  const res = await POST(makeRequest({ key: 'value' }))
  expect(res.status).toBe(200)
  const body = await res.json()
  expect(body).toMatchObject({ received: true })
})
```

### Fixtures

Put shared test data in a `helpers/fixtures.ts` file in the test directory:

```typescript
// Use real valid values — Zod schemas validate UUIDs, emails, etc.
// Generate valid v4 UUIDs with: node -e "const { randomUUID } = require('crypto'); console.log(randomUUID())"

export const mockUser = {
  id: 'f47ac10b-58cc-4372-a567-0e02b2c3d479',  // valid v4 UUID
  email: 'test@example.com',
  name: 'Test User',
  // ...
}
```

**Critical**: If your code validates UUIDs with `z.string().uuid()`, the fixture IDs **must** be real RFC-4122 v4 UUIDs. Convenient-looking IDs like `'user-uuid-1234'` or `'11111111-1111-1111-1111-111111111111'` will be rejected by Zod v4's strict UUID validator (which requires `[1-8]` version digit and `[89abAB]` variant bits). Generate real ones.

---

## Step 6 — Run the tests and fix failures

After writing, always run the tests:

```bash
npm test
# or
npx vitest run
```

**Common failure patterns and fixes:**

| Error | Cause | Fix |
|---|---|---|
| `Cannot find module './helpers/mocks'` | `require()` inside hoisted `vi.mock()` factory | Move spies to module scope, close over them in the factory |
| `Invalid UUID` from Zod | Fixture ID like `'user-uuid-1'` fails Zod v4's strict UUID regex | Generate real v4 UUIDs with `crypto.randomUUID()` |
| `TypeError: X is not a function` | Mock chain missing a method the handler calls | Add the missing method to the mock chain |
| `expected false to be true` (happy path) | Validation failing before the test's intended code path | Check what `result.error` says — likely a fixture value fails schema validation |
| `expected X to be called 0 times, got 1` | `clearMocks: true` not set, previous test's call bleeds over | Add `clearMocks: true` to vitest.config.ts |
| Mock resolved wrong value | `mockReturnValue` instead of `mockResolvedValue` on the terminal chain method | The method that's `await`ed needs `mockResolvedValue`, not `mockReturnValue` |
| Handler hits generic catch instead of typed catch | Error thrown inside `cb(tx)` but transaction mock swallows it | Ensure `return cb(tx)` in the transaction mock (not `await cb(tx)` in a `try/catch`) |

---

## Step 7 — Present results

After all tests pass, show the user:

1. **Test count** — how many tests were written and are passing
2. **Coverage areas** — what scenarios are now covered (happy path, validation, guards, error paths)
3. **How to run** — the exact command (`npm test`, `npx vitest run`, etc.)
4. **Any gaps** — scenarios you deliberately skipped and why (e.g. "end-to-end email delivery not tested — would need a real SMTP server")

---

## Configuration Reference

### Minimal `vitest.config.ts` for a Next.js / Node project

```typescript
import { defineConfig } from 'vitest/config'
import tsconfigPaths from 'vite-tsconfig-paths'

export default defineConfig({
  plugins: [tsconfigPaths()],  // resolves @/ path aliases
  test: {
    environment: 'node',       // server-side code — no DOM needed
    isolate: true,             // each test file gets its own module scope
    clearMocks: true,          // clears call history between tests (not implementations)
    restoreMocks: true,        // restores spied originals after each test
    include: ['src/tests/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.ts'],
      exclude: ['src/tests/**', 'src/**/*.d.ts'],
      reporter: ['text', 'html'],
    },
  },
})
```

**Note**: `clearMocks: true` clears call counts/args but NOT implementations set with `mockImplementation`. Each test's `beforeEach` should re-apply any implementations it needs.

### When you also need browser/DOM (React components)

```typescript
// vitest.config.ts
export default defineConfig({
  plugins: [tsconfigPaths()],
  test: {
    // Use 'jsdom' or 'happy-dom' for component tests
    environment: 'jsdom',
    setupFiles: ['./src/tests/setup.ts'],  // import @testing-library/jest-dom here
    // ...
  },
})
```

---

## What this skill does NOT do

- **Does not write e2e tests** — if you want browser-driven tests (Playwright, Cypress), ask explicitly
- **Does not test React component rendering** — for component tests with `@testing-library/react`, ask explicitly
- **Does not test third-party library internals** — only tests your application code against mocked boundaries
- **Does not guarantee 100% coverage** — aims for meaningful coverage of real application logic, not line-count coverage metrics
