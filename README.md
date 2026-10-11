# Rust Store API

[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust)](https://www.rust-lang.org)
[![Axum](https://img.shields.io/badge/Axum-0.8-blue)](https://github.com/tokio-rs/axum)
[![SurrealDB](https://img.shields.io/badge/SurrealDB-3.3_embedded-ff00a0)](https://surrealdb.com)
[![Release](https://img.shields.io/github/v/release/akanyane/rust-store-api?include_prereleases&sort=semver)](https://github.com/akanyane/rust-store-api/releases)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-green)](LICENSE)

A small online-store API written in Rust: catalog with variants, cart,
authentication with refreshable sessions, transactional checkout, simulated
payment and an admin surface for products and orders. There is **no external
database to run**: SurrealDB is embedded and stores everything under `data/`.

It is built for **simulated purchases**. There is no payment gateway, no email
and no real money, and the store's currency is the **unit** (a whole number,
no subunits).

The web storefront and admin console that talk to this API live in
[rust-store-web](https://github.com/akanyane/rust-store-web).

## Highlights

- **Transactional checkout**: stock is checked and decremented in one atomic
  command, so two buyers racing for the last unit never oversell. The loser gets
  a `409`, never a `500`.
- **Sessions you can actually revoke**: 1-hour session token plus a single-use
  15-day refresh token that rotates; only SHA-256 hashes of tokens are stored,
  passwords use argon2.
- **Soft deletes**: products and variants are deactivated, not erased, because
  carts and orders point at them.
- **Order snapshots**: an order keeps the name, SKU and price from the moment of
  purchase.
- **Admin surface**: list and filter all orders, move them through the status
  cycle, and manage inactive products and variants.
- **Input validation** with no echo of what was sent (privacy by default).
- **Integration tests** that boot the whole app against a throwaway database.

## Table of contents

- [Quick start](#quick-start) · [Environment variables](#environment-variables)
- [Try it](#try-it)
- [Routes](#routes)
- [Authentication](#authentication)
- [Catalog](#catalog): [soft delete](#soft-delete), [ordering](#ordering),
  [price range](#price-range), [pagination and totals](#pagination-and-totals),
  [creation date](#creation-date)
- [Cart and checkout](#cart-and-checkout)
- [Orders and payment](#orders-and-payment)
- [Admin dashboard numbers](#admin-dashboard-numbers)
- [Conventions](#conventions): currency, validation
- [Project layout](#project-layout) · [Tests](#tests) · [Known limitations](#known-limitations)

## Quick start

You need a recent stable Rust toolchain (the crate uses the 2024 edition).

```bash
cp .env.example .env     # then set ADMIN_USERNAME and ADMIN_PASSWORD if you want an admin
cargo run
```

The server listens on `http://localhost:3000`. The database is created at
`data/rust-store` (git-ignored) and the schema (`schema.surql`) is applied on
every start.

### Environment variables

| Variable | Required | Description |
|---|---|---|
| `PORT` | no | HTTP port. Defaults to `3000`. |
| `ADMIN_USERNAME` | no* | Username of the admin created on startup. |
| `ADMIN_PASSWORD` | no* | Password of the admin created on startup. |

\* Set `ADMIN_USERNAME` and `ADMIN_PASSWORD` together, or neither. An empty
value counts as missing, and the store then starts without an admin. Creation is
idempotent: it never changes the password of an existing admin and never
promotes an existing regular user.

## Try it

A full purchase from the terminal (admin credentials as set in `.env`; the
ids in the responses are random strings, so copy them as you go):

```bash
API=http://localhost:3000

# 1. Admin signs in and creates a product (a "Default" variant is created with it)
ADMIN=$(curl -s $API/auth/sign-in -H 'content-type: application/json' \
  -d '{"username":"admin","password":"<ADMIN_PASSWORD>"}' | jq -r .session_token)
curl -s $API/products -H "authorization: Bearer $ADMIN" -H 'content-type: application/json' \
  -d '{"name":"Ceramic mug","description":"Blue","price":25,"stock":10}'

# 2. A customer signs up, and finds the variant id
curl -s $API/auth/sign-up -H 'content-type: application/json' \
  -d '{"username":"ana","password":"a-long-password","first_name":"Ana","last_name":"Lima","birthday":"1990-01-01"}'
TOKEN=$(curl -s $API/auth/sign-in -H 'content-type: application/json' \
  -d '{"username":"ana","password":"a-long-password"}' | jq -r .session_token)
curl -s $API/products            # note the product id
curl -s $API/products/<product_id>/variants   # note the variant id

# 3. Cart, checkout, simulated payment
curl -s $API/cart/items -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"variant_id":"<variant_id>","quantity":2}'
curl -s -X POST $API/orders -H "authorization: Bearer $TOKEN"
curl -s -X POST $API/orders/<order_id>/pay -H "authorization: Bearer $TOKEN"
```

## Routes

Access is **public**, **customer** (Bearer token of a customer) or **admin**
(Bearer token of an admin).

| Method | Route | Access | Description |
|---|---|---|---|
| GET | `/health` | public | Health check |
| POST | `/auth/sign-up` | public | Customer sign-up (`username`, `password`, names and birthday) |
| POST | `/auth/sign-in` | public | Sign in; returns `session_token` and `refresh_token` |
| POST | `/auth/sign-out` | public | Ends the session, given the `refresh_token` |
| POST | `/auth/refresh` | public | Exchanges a `refresh_token` for a new pair of tokens |
| GET | `/auth/session` | customer or admin | Who is signed in: `id`, `username` and `role` (`customer` or `admin`) |
| GET | `/products` | public | Lists active products (`?sort=`, `?limit=`, `?offset=`; total in `X-Total-Count`; with `price_from` and `price_to`) |
| POST | `/products` | admin | Creates a product |
| GET | `/products/{id}` | public | Product detail |
| PUT | `/products/{id}` | admin | Replaces the product's name, description and `active` |
| DELETE | `/products/{id}` | admin | Deactivates the product (soft delete) |
| GET | `/products/{id}/variants` | public | Lists the product's variants |
| POST | `/products/{id}/variants` | admin | Creates a variant |
| PUT | `/products/{id}/variants/{variant_id}` | admin | Replaces the variant's name, SKU, price, stock and `active` |
| DELETE | `/products/{id}/variants/{variant_id}` | admin | Deactivates the variant (soft delete) |
| GET | `/admin/products` | admin | Lists active and inactive products (`?active=`, `?sort=`, `?limit=`, `?offset=`; total in `X-Total-Count`) |
| GET | `/admin/products/{id}` | admin | Product detail (even if inactive) with every variant, inactive ones included |
| GET, PUT, DELETE | `/me` | customer | Reads, updates and deletes the customer's own profile |
| GET | `/cart` | customer | Reads the cart |
| POST | `/cart/items` | customer | Adds a variant to the cart |
| PUT, DELETE | `/cart/items/{variant_id}` | customer | Changes the quantity or removes an item |
| GET | `/orders` | customer | Lists the customer's orders |
| POST | `/orders` | customer | Checks out the cart |
| GET | `/orders/{id}` | customer | One of the customer's orders |
| POST | `/orders/{id}/cancel` | customer | Cancels a pending order and returns the stock |
| POST | `/orders/{id}/pay` | customer | Simulated payment of a pending order |
| GET | `/admin/orders` | admin | Lists every customer's orders (`?status=`, `?q=`, `?limit=`, `?offset=`; total in `X-Total-Count`) |
| GET | `/admin/orders/{id}` | admin | Any order |
| PUT | `/admin/orders/{id}/status` | admin | Changes the order's status |
| GET | `/admin/stats` | admin | Numbers for the admin dashboard: orders per status, revenue, product counts and low stock |

## Authentication

Protected routes expect `Authorization: Bearer <session_token>`.

Sign-in is by `username` and password, with no email. The `username` is
case-insensitive (`Ana` and `ana` are the same user) and stored in lowercase.

- A session lasts 1 hour and a refresh token 15 days.
- `POST /auth/refresh` rotates the tokens: a refresh token works only once, and
  the old session stops working.
- `GET /auth/session` says who is signed in and with which role. `/me` is for
  customers only (it returns 404 for an admin, who has no profile); the session
  route is how a frontend knows whether to show the admin area.
- Sessions whose refresh token has expired are deleted on startup and then every
  hour.
- Passwords are stored with argon2; tokens only as SHA-256 hashes.
- The customer is always identified by the token, never by an id in the URL.

## Catalog

### Soft delete

`DELETE` on a product or variant does not erase the record: it sets
`active = false`, because orders and carts point at them. Inactive items vanish
from the public catalog (`GET /products`, `/products/{id}`,
`/products/{id}/variants`) and cannot enter a cart or a checkout. To reactivate
one, `PUT` it with `"active": true`; find the ids of deactivated items with
`GET /admin/products?active=false` (products) and `GET /admin/products/{id}`
(variants, with each one's `active`). The `stock` in a `PUT` is an absolute
value, so a concurrent checkout may be overwritten.

### Ordering

`GET /products` returns active products by name and, on a tie, by id. With
`?sort=newest` they come from newest to oldest, and products without a creation
date go last; `?sort=name` is the default. An unknown `sort` returns `422`, and
parameters the route does not know are ignored. The admin list accepts the same
`sort`.

Variants (`GET /products/{id}/variants`, the `variants` of the public detail and
of the admin detail) come by name and, on a tie, by SKU (which is unique, so the
order is always the same). They have no `sort` or pagination. Name ordering uses
SurrealDB's Unicode collation (`COLLATE`): it ignores case differences and sorts
accented letters next to their base letters (`Álamo`, `alpha`, `Beta`, `zebra`,
`Zed`). It does **not** do natural ordering of numbers inside text (`Item 10`
comes before `Item 2`) and has no locale-specific tuning.

### Price range

Each product in `GET /products` carries `price_from` (the lowest price among its
**active** variants) and `price_to` (the highest), in units. A client can show
"from X" only when the two differ. Inactive variants do not count, because they
cannot be bought; zero stock does not remove a variant from the calculation. A
product with no active variant has both as `null`. The fields are added to the
usual shape (`id`, `name`, `description`, `active`, `created_at`); only the
public list has them.

### Pagination and totals

Pagination is **optional** on the public list: without `limit`, `GET /products`
returns every active product (there is no default limit that would cut the
catalog). With `?limit=` (1 to 200) and `?offset=` (from 0) it returns a page;
`offset` alone returns the rest from that point. Pages follow `sort` and never
include inactive products. Invalid values return `422`.

The body stays a plain list, and the `X-Total-Count` header carries how many
items exist in total, regardless of the page (`0` if there are none). The
admin lists (`GET /admin/products` and `GET /admin/orders`) send it too, and
there the total **respects the filter** (`?active=` on products, `?status=` on
orders) and **ignores `limit` and `offset`**. The count and the page are two
separate reads, so a record created between them can leave the total off by one.

### Creation date

Every product has a `created_at`, written by the application on creation, that
never changes afterwards (editing, deactivating and reactivating keep it).
Products created **before** the field existed have `created_at: null`, because
the real date was not stored and is not invented; under `?sort=newest` they come
last.

## Cart and checkout

**Names on items.** Cart and order items carry `name` (the **variant's** name;
the default variant, created together with the product, is called `Default`) and
`product_name` (the **product's** name). In the **cart** `product_name` is the
current name, so renaming a product shows up at once. In **orders** it is a
snapshot from the moment of purchase, like the variant name, the SKU and the
price: renaming the product later does not change an order already placed. This
applies to customer and admin responses. Order items created **before** the field
existed have `product_name: null`, because the name from that time was not stored
and is not invented.

**Unavailable items.** Cart responses (`GET /cart`, `POST /cart/items`, `PUT` and
`DELETE /cart/items/{variant_id}`) flag what cannot be bought right now:

- each item has `available` and `unavailable_reason`: `null` when available,
  `"inactive"` if the product or variant was deactivated, `"insufficient_stock"`
  if the current stock is lower than the quantity (including zero stock; a
  quantity equal to the stock is still valid);
- the cart has `can_checkout`: `true` only if it has items and all of them are
  available.

The `total` still adds up every item, available or not, and no item is removed
on its own. It is a snapshot: the checkout is what decides, and it still refuses
with `409` if something changed between reading the cart and buying.

**Checkout.** `POST /orders` runs in a single transaction: it re-validates that
the variant and product are active, decrements the stock
(`UPDATE ... WHERE stock >= quantity`), creates the `pending` order with a
snapshot of each item's name, SKU and price, and empties the cart. If any step
fails, nothing is written. Simultaneous purchases of the same item collide on
write; the losing transaction is retried automatically (up to 5 attempts, with a
random pause), so whoever loses the last unit gets a `409` and never a `500`.
Cancelling an order uses the same retry.

## Orders and payment

**Status cycle:** `pending` → `paid` or `cancelled`; `paid` → `shipped` →
`delivered`. `cancelled` and `delivered` are final. `PUT /admin/orders/{id}/status`
takes `{"status": "paid"}` and returns `409` for any transition outside the
cycle. Cancelling a pending order returns the stock; a paid order cannot be
cancelled (there are no refunds). Customers can still pay and cancel their own
orders, but only while the order is `pending`.

```
pending ──► paid ──► shipped ──► delivered
   │
   └──► cancelled
```

**Payment is simulated.** `POST /orders/{id}/pay` only switches the status from
`pending` to `paid`, with no gateway and no card data. Stock does not change (it
was decremented at checkout). Only a pending order can be paid; a `paid` or
`cancelled` one returns `409`. There are no refunds, so a paid order **cannot be
cancelled**. Paying and cancelling at the same time has a single winner.

**Payment date.** Every order has `paid_at`: the instant it became `paid`,
written by the database in the same command that changes the status (both on
`POST /orders/{id}/pay` and on `PUT /admin/orders/{id}/status`). It is `null`
until the order is paid and on cancelled orders. Shipping and delivering do not
change it. Orders paid **before** the field existed have `paid_at: null`, because
the real date was not stored and is not invented.

**Admin list.** `GET /admin/orders` lists from newest to oldest, with `limit`
(1 to 200, default 50) and `offset`. Each order carries only the owner's
`customer_id`, with no personal data. Invalid parameters return `422`.

**Search.** `?q=` finds orders by the **order number** shown on screen (the first 8
characters of the id, with or without the `#`, or the whole id) or by the
**customer id**, always as a **prefix** and ignoring case, surrounding spaces and a
leading `#`. So `?q=%239FST3BCC`, `?q=9fst3bcc` and the full id all find the same
order, and a customer id (or its start) finds all of that customer's orders. It
combines with `status`, and `X-Total-Count` counts the whole search, regardless of
`limit` and `offset`. An empty or blank `q` means no search, a term with no match
is an empty list with a total of `0`, and a term over 64 characters is a `422`.
The term is only ever compared as text (`%`, quotes and `*` have no special
meaning). There is no search by customer name: the admin never sees one.

## Admin dashboard numbers

`GET /admin/stats` returns what a dashboard needs in one call:

```json
{
  "orders": { "total": 5, "pending": 1, "paid": 1, "shipped": 1, "delivered": 1, "cancelled": 1 },
  "revenue": 60,
  "products": { "active": 3, "inactive": 1 },
  "low_stock": [
    { "product_id": "…", "product_name": "Tote", "variant_id": "…", "variant_name": "Large", "sku": "TOTE-L", "stock": 2 }
  ]
}
```

- `revenue` is the sum of `total` of orders that are `paid`, `shipped` or `delivered`, in units. `pending` and `cancelled` orders are not revenue.
- `low_stock` lists **active** variants of **active** products with **5 units or fewer** (zero included), emptiest first and then by SKU, at most **10**. The threshold and the limit are fixed in the service.
- With no orders or products the counts are `0` and `low_stock` is `[]`.
- The counts and the list are separate reads, so a sale made in between can leave one number off by one relative to another.

## Conventions

**Currency.** The store runs on **units** (singular: *unit*), always whole
numbers with no subunit. The value fields are `price`, `unit_price`, `line_total`
and `total` (e.g. `"price": 25` = 25 units). The whole API answers in English.

**Validation.** JSON bodies go through the `ValidatedJson` extractor (the
`validator` crate). A malformed body or a broken rule returns `422` with
`{"error": "field: message"}`, never echoing the value that was sent (LGPD, the
Brazilian data-protection law). Rules: username of 3 to 32 characters (letters,
digits, `.`, `-` and `_`), non-empty names (max 100), non-negative price and
stock, quantity >= 1, birthday not in the future, password of 8 to 128
characters.

## Project layout

```
src/
├── main.rs         # routes and startup
├── db.rs           # connection and schema
├── state.rs        # AppState
├── error.rs        # AppError and HTTP responses
├── executor.rs     # abstraction over connection and transaction
├── extractors.rs   # AuthCustomer, AuthAdmin, AuthUser, ValidatedJson
├── handlers/       # HTTP layer
├── services/       # business rules
├── repositories/   # SurrealDB access
├── models/         # input and output types
└── tests/          # integration tests
schema.surql        # table definitions
```

## Tests

```bash
cargo test
```

The tests boot the whole app (routes, services and schema) against a fresh
SurrealKV database in a temporary directory and call the `Router` in memory,
without opening a port. They never touch `data/rust-store`. They cover
authentication (including concurrent refresh), the catalog (including the admin
view of inactive items), the cart (including unavailable items), checkout
(rollback and the race for the last item), cancellation, payment, the admin
order routes (including concurrent ones) and session cleanup.

## Known limitations

- There is no real payment gateway and no refunds.
- Variants have no pagination, and numbers inside names are not ordered naturally.

## Contributing and releases

Commits follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat:`, `fix:`, `docs:`, and `!` or a `BREAKING CHANGE:` footer for breaking
changes). [Release Please](https://github.com/googleapis/release-please-action)
reads them on every push to `main`, keeps a release pull request open with the
next version and the `CHANGELOG.md`, and tags and publishes the release when that
pull request is merged.

## License

[The Unlicense](LICENSE): public domain, do whatever you want with it.
