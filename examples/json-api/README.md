# json-api

JSON API canister with GET/POST/PUT/DELETE method routing and CORS middleware.

## Features demonstrated

- JSON endpoints returning `application/json` responses
- HTTP method routing: `GET`, `POST`, `PUT`, `DELETE` on the same path
- Typed route parameters (`_itemId` directory → `Params { item_id: String }`)
- Root-level CORS middleware (`middleware.rs`) that adds `Access-Control-Allow-*` headers
- In-memory data store with CRUD operations

## Routes

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Welcome message with endpoint list |
| GET | `/items` | List all items |
| POST | `/items` | Create a new item (`{"name":"..."}`) |
| GET | `/items/:itemId` | Get an item by ID |
| PUT | `/items/:itemId` | Update an item (`{"name":"..."}`) |
| DELETE | `/items/:itemId` | Delete an item |

## Project structure

```
src/
  lib.rs                  Canister entry points
  data.rs                 In-memory item storage (thread-local Vec)
  routes/
    index.rs              GET /
    middleware.rs          CORS middleware (root scope)
    items/
      index.rs            GET /items, POST /items
      _itemId/
        index.rs          GET/PUT/DELETE /items/:itemId
build.rs                  Route tree generation
```

## Run

```
dfx start --background
dfx deploy
```

Test with curl:

```
# List items
curl http://localhost:4943/?canisterId=<id>

# Create item
curl -X POST -H 'Content-Type: application/json' \
  -d '{"name":"New item"}' \
  http://localhost:4943/items?canisterId=<id>
```
