# api-authentication

Demonstrates two patterns for authenticated API endpoints on the Internet Computer, with different security and performance trade-offs.

## The problem

IC HTTP certification protects users against malicious replicas that could tamper with responses. For authenticated endpoints, the question is: should the certification proof include the caller's identity?

- **Full certification:** Each caller gets a cryptographically certified response bound to their Authorization header. Secure, but every request must go through a slower update call (~2s).
- **Skip + handler auth:** The handler runs on every query call (like a candid query), checks credentials, and returns 401 if missing. Fast (~200ms), same security model as candid query calls.

## Security model comparison

|  | Caller authenticated | Response tamper-proof | Performance |
|--|---|---|---|
| Candid query call | Yes (principal verified) | No (trust replica) | Fast (~200ms) |
| Candid update call | Yes | Yes (consensus) | Slow (~2s) |
| HTTP skip + handler auth | Application-level (header check) | No (trust replica) | Fast (~200ms) |
| HTTP response-only | No | Yes (certified) | Fast (cached query) |
| HTTP full (authenticated) | Bound to request headers | Yes (certified) | Slow (~2s update) |

**Key insight:** Skip certification has the same response trust level as a candid query call. In both cases, you trust the replica to return honest data. If candid query calls are acceptable for your application, skip certification is equally acceptable.

## Routes

| Method | Path | Certification | Auth | Description |
|--------|------|---------------|------|-------------|
| GET | `/` | Response-only | None | About page explaining both patterns |
| GET | `/profile` | Full (`authenticated`) | Required | Per-user profile — response varies per caller |
| GET | `/customers` | Skip | Handler-checked | Shared customer list — auth checked every call |

## When to use which pattern

**Use full certification** (`#[route(certification = "authenticated")]`) when:
- The response depends on who is calling (user profiles, account settings)
- Serving one user's response to another is a security issue
- You need cryptographic proof that the response matches the request

**Use skip + handler auth** (`#[route(certification = "skip")]`) when:
- The endpoint needs auth checking on every call
- Performance matters (avoid ~2s update call overhead)
- The response is dynamic or shared among authenticated users
- The same trust model as candid query calls is acceptable

## Project structure

```
src/
  lib.rs                  Canister entry points
  routes/
    index.rs              GET / — about page with pattern comparison
    profile.rs            GET /profile — full certification (authenticated)
    customers.rs          GET /customers — skip + handler auth
build.rs                  Route tree generation
```

## Run

```
dfx start --background
dfx deploy
```

Test with curl:

```sh
# Public page
curl http://<canister-id>.localhost:4943/

# Full certification — each user gets a separate update call (~2s each)
curl -H 'Authorization: Bearer alice-token' http://<canister-id>.localhost:4943/profile
curl -H 'Authorization: Bearer bob-token' http://<canister-id>.localhost:4943/profile

# Skip + handler auth — fast query call, auth checked every time
curl -H 'Authorization: Bearer alice-token' http://<canister-id>.localhost:4943/customers
curl -H 'Authorization: Bearer bob-token' http://<canister-id>.localhost:4943/customers

# No auth — both endpoints return 401
curl http://<canister-id>.localhost:4943/customers
curl http://<canister-id>.localhost:4943/profile
```
