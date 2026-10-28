# Example shop: orders, refunds and receipts

A small order shop API: customers browse products, place orders, get refunds through a payment provider and e-mail receipts; admins delete users. It shows, in five minutes, what Sezgi learns from code alone.

Plain JavaScript (Express, zod, nodemailer) with an in-memory SQLite database (sql.js, no native build) and seeded demo data. Refunds and e-mails are only logged unless `PAYMENTS_API_KEY` / `SMTP_URL` are set.

| Operation | Who |
|---|---|
| `GET /api/products` | anyone |
| `GET /api/orders/:id` | signed in, own order |
| `POST /api/orders` | signed in |
| `POST /api/orders/:id/refund` | signed in, own order |
| `POST /api/orders/:id/email-receipt` | signed in, own order |
| `POST /api/account/change-password` | signed in |
| `DELETE /api/users/:id` | admin |

Tokens: `demo-customer-token`, `demo-admin-token` (`Authorization: Bearer ...`).

## Five-minute walkthrough

```sh
npm install && npm start          # http://127.0.0.1:3000 (Node 18+)
sezgi init .                      # another terminal: writes .sezgi/
sezgi mirror                      # (sezgi ayna) the map in your browser
sezgi report                      # what is known and what is not
```

What to look at in the mirror (or in `.sezgi/kapilar.json`):

- **products**: public, reads `products`.
- **GET order**: login from `requireAuth` (answers 401); `order.user_id !== req.user.id` is an ownership check.
- **POST order**: fields and rules from the zod schema (`quantity` int, 1..10); reads `products`, writes `orders`.
- **refund**: outgoing `HTTP POST api.payments.example.com/refunds`, found through `services/payments.js`; cannot be undone, so always waits for approval.
- **email-receipt**: e-mail through nodemailer (`services/mailer.js`); always waits for approval.
- **change-password**: writes `users`; flagged as a credential change.
- **delete user**: role `admin` from `requireRole('admin')`; destructive; `orders` deleted via `ON DELETE CASCADE` in `schema.sql`.

Then `sezgi connect` (`sezgi bagla`) and ask your assistant, for example:

- "Which operations reach outside this app?"
- "What happens to a user's orders when the user is deleted?"
- "Which operations need a human's approval, and why?"

## Optional: live mode against a test copy

With `npm start` running as the test copy:

```sh
sezgi live setup --hedef http://127.0.0.1:3000 --test-kopyasi --mod yazma --izin "orders.*:yaz"
```

When asked for your session, paste `demo-customer-token`. Operation ids are `orders.*`, `account.*`, `users.*`, `products.*`. Ask the assistant to place an order, then refund it: the refund waits for `sezgi approve show <id>` and `sezgi approve give <id>`. Finish with `sezgi session close`.

## Expected result

From `sezgi init` (0.1.0) on this folder:

- 7 operations · 6 need login · 1 public · 1 role-restricted (`admin`).
- Inputs: 2 operations, 4 fields, 8 rules, all from zod.
- Data: 7 / 7 operations linked to a table; cascade `users` → `orders` found.
- External: `http api.payments.example.com` (refund), `e-mail smtp / nodemailer` (email-receipt).
- Ownership checks on 3 operations.
- Undo: 3 writes irreversible, 2 unknown. 4 operations always need approval (`etki_onayi`): refund (external HTTP), email-receipt (e-mail), delete user (hard delete), change-password (credential change).
- `POST /api/orders` also waits for approval: Express login is inferred, not certain, and the preview cannot check its 404 for an unknown product.

Per-operation summary: `beklenen.json`.
