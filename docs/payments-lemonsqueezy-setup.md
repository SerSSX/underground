# Live payments + escrow (Lemon Squeezy) — setup guide

TASK-017 M6 ships the **full escrow + payout pipeline** — receive-side Lemon Squeezy
webhook (HMAC-SHA256 verified), time-based escrow clearing, organizer payouts, buyer
disputes, and organizer verification — all tested to 100%. Test mode works out of the box;
this guide enables **live** payments.

## Escrow lifecycle

```
checkout → order(pending) ──pay──▶ held ──event over──▶ clearing ──+window──▶ cleared ──admin payout──▶ paid_out
                                     │                      │
                              buyer dispute / admin freeze ─┴──▶ frozen ──admin──▶ refunded | cleared
```

- **held** — funds escrowed at payment (test pay or live webhook).
- **clearing** — the event is over; funds are releasing (set by the hourly `escrow::sweep`).
- **cleared** — the clearing window (`clearing_window_days`, default 5) elapsed without a
  dispute; eligible for payout.
- **paid_out** — an admin issued a payout (terminal). Money only reaches an organizer this
  way — never automatically — so "collect-and-vanish" is impossible.
- **frozen** — a buyer dispute (`POST /api/orders/:id/dispute`) or admin freeze; the sweep
  skips it until an admin refunds or releases.

The clearing sweep runs as a background worker (`main.rs`); it is also unit-callable
(`escrow::sweep(db, days)`) and fully covered by tests.

## What works without setup
- Test-mode checkout → `pay-test` → ticket + escrow held.
- The whole escrow lifecycle, payouts, disputes, verification (admin `/payouts`, `/users`).
- The webhook endpoint exists; it returns **409** until a webhook secret is configured.

## Enabling live Lemon Squeezy

1. **Create a Lemon Squeezy store** + a product **variant** for ticket purchases.
2. In the platform admin (`/payments`) set:
   - `mode = live`, `provider = lemonsqueezy`
   - `ls_store_id` = your store slug, `ls_variant_id` = the variant id
   - `ls_api_key` = your API key (stored masked; never returned by GET)
   - `ls_webhook_secret` = the signing secret you set on the LS webhook (masked)
   - optionally `platform_fee_bps` (default 1000 = 10%) and `clearing_window_days` (default 5)
3. **Point the LS webhook** at `POST https://<your-host>/api/webhooks/lemonsqueezy` and
   subscribe to `order_created` / `order_paid`. LS signs each delivery with the secret;
   the API verifies the `X-Signature` HMAC-SHA256 over the raw body (constant-time) before
   acting — an invalid/missing signature is rejected **401** before any parsing.
4. **Checkout flow (live):** `POST /api/events/:id/checkout` returns a `checkout_url` —
   the LS hosted checkout with the order id as `checkout[custom][order_id]`. The buyer pays
   on LS; LS fires the webhook; we match `meta.custom_data.order_id` back to the pending
   order and fulfill it (mint ticket + escrow held). Fulfillment is **idempotent** —
   duplicate deliveries never double-mint.

## Payouts

`POST /api/admin/organizers/:id/payout` settles all **cleared** escrow for an organizer:
one payout row per currency, amount = `sum · (10000 − platform_fee_bps) / 10000` (net of
commission), and those orders flip to `paid_out`. History at `GET /api/admin/payouts`
(admin `/payouts` page). Actual bank disbursement is out-of-band (LS is MoR; it can't
auto-split), so a payout row is the platform's settlement record.

## Notes
- The webhook is **receive-only**; the platform does not call the LS API to create
  checkouts server-side (the hosted-checkout URL is constructed client-redirect-style). A
  server-side LS checkout-create call (for embedded checkout) is a follow-up needing `reqwest`.
- `ls_api_key` / `ls_webhook_secret` are write-only via PUT and exposed only as `*_set`
  booleans in GET — never returned.
