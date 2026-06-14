# Privacy Policy — underground

**TEMPLATE — review with legal counsel before publishing. Last updated: 2026-06-13.**

underground ("we") operates an events discovery, ticketing and social platform. This policy
explains what personal data we process and your rights under the GDPR.

## Data we collect
- **Account:** email, display name, password (hashed with argon2id — never stored in plaintext),
  optional bio, avatar, and date of birth (used only for age-gating restricted events).
- **Usage:** events you create/save, tickets, orders, reviews, posts, comments, follows, messages.
- **Payments:** order amount, currency, status, and Lemon Squeezy payment identifiers. We are a
  marketplace using Lemon Squeezy as Merchant of Record; card data is processed by Lemon Squeezy,
  not stored by us.
- **Organizer payouts:** for organizers requesting payouts we collect KYC identity + bank details
  (name, IBAN, SWIFT/BIC) for anti-money-laundering compliance.
- **Devices:** push notification tokens (if you opt in).

## How we use it
To provide the service (discovery, ticketing, escrow, payouts), to notify you (in-app and, if
enabled, email/push), to prevent abuse and fraud, and to meet legal/tax obligations.

## Retention
Account and content data are kept while your account is active. **Financial records (orders,
tickets, payouts) are retained for up to 7 years** to meet tax/audit obligations even after
account deletion — but are de-linked from your identity (see "Your rights").

## Your rights (GDPR)
- **Access / portability (Art. 15 & 20):** download your data via *Settings → Export my data*
  (`GET /api/me/export`).
- **Erasure (Art. 17):** delete your account via *Settings → Delete my account*
  (`DELETE /api/me`, re-authentication required). We anonymize your identity and delete your
  personal content; financial records are retained but de-identified.
- **Rectification (Art. 16):** edit your profile at any time.
- Contact our DPA at **privacy@underground.app** for any request.

## Sharing
We share data only with processors needed to run the service: Lemon Squeezy (payments),
the email provider (transactional email), the push provider (notifications), and the KYC
provider (organizer verification). We do not sell personal data.

## Security
TLS in transit; argon2id password hashing; payment secrets are write-only (never returned by the
API); admin actions are access-controlled.
