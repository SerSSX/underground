//! Stripe adapter behind [`crate::provider::PaymentProvider`] (PAY-4).
//!
//! Shape: everything that can be a pure function IS one — minor-unit conversion, form-body
//! construction, signature verification, response parsing. Only `send` touches the network.
//! That split is not stylistic: the 100%-line gate cannot cover an adapter whose logic is
//! welded to an HTTP call, and a money bug in unit conversion is exactly the class of defect
//! that a network-dependent test suite quietly skips.
//!
//! Money model: **direct charges**. The payment is created ON the organiser's connected
//! account (`Stripe-Account` header), so the funds land on their balance and we take our cut
//! as `application_fee_amount` in the same movement. The platform never holds the buyer's
//! money — see `docs/backlog/2026-09-06-monetisation/`.

use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::Sha256;

use crate::provider::{
    ChargeReq, ChargeResult, ChargeStatus, InboundWebhook, PaymentProvider, PayoutReq, PayoutStatus, ProviderError,
    RefundReq, RefundResult, RefundStatus, TransferResult, WebhookEvent,
};

type HmacSha256 = Hmac<Sha256>;

/// Stripe's live API.
pub const DEFAULT_BASE_URL: &str = "https://api.stripe.com";

/// The API root to talk to. Overridable ONLY through the environment, so a test can point
/// the adapter at a local mock; nothing in the request path can redirect it, which keeps a
/// compromised settings row from exfiltrating the secret key to another host.
///
/// The override must be `https://…`, or loopback `http://` for a local mock: the secret key
/// travels as a bearer header, and a typo like `http://internal-proxy` in a compose file would
/// send it in clear. An unusable override is refused, not silently replaced with the default
/// — an operator who set it meant something, and the wrong host quietly is worse than none.
pub fn base_url() -> Result<String, ProviderError> {
    let Some(raw) = std::env::var("STRIPE_API_BASE")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
    else {
        return Ok(DEFAULT_BASE_URL.to_string());
    };
    let loopback =
        raw.starts_with("http://127.0.0.1") || raw.starts_with("http://localhost") || raw.starts_with("http://[::1]");
    if raw.starts_with("https://") || loopback {
        Ok(raw)
    } else {
        Err(ProviderError::Invalid(
            "STRIPE_API_BASE must be https:// (or http:// on loopback for a local mock)".into(),
        ))
    }
}

/// One HTTP client for the process. `provider::resolve` runs on every checkout and every
/// webhook; building a client each time meant a fresh connection pool and TLS handshake per
/// payment, and idle sockets piling up under load.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        // A timeout is not optional: without one a hung Stripe call holds the request task
        // (and its database connection) until the client gives up.
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest client builds with default TLS")
    })
}

/// Attempts for a request that is safe to repeat (it carries an idempotency key). Stripe
/// asks for backoff on 429 and 5xx; three tries covers a blip without turning a real outage
/// into a minute of hung checkouts.
const MAX_ATTEMPTS: u32 = 3;

/// Largest response body we will parse. Stripe objects are kilobytes; a megabyte from a host
/// we were misrouted to is not a response, it is an attack surface.
const MAX_RESPONSE_BYTES: usize = 1 << 20;

/// How far a webhook's own timestamp may be from our clock. Stripe's documented default;
/// without it a captured-and-replayed delivery stays valid forever, because the signature
/// itself never expires.
const SIGNATURE_TOLERANCE_SECS: i64 = 300;

/// Currencies Stripe quotes with NO minor unit. Sending 1000 for ¥1000 charges ¥100 000 —
/// a silent factor-of-100 error that reconciles to a plausible-looking number, which is why
/// this list is explicit rather than derived.
const ZERO_DECIMAL: [&str; 15] = [
    "bif", "clp", "djf", "gnf", "jpy", "kmf", "krw", "mga", "pyg", "rwf", "ugx", "vnd", "vuv", "xaf", "xof",
];

/// Currencies Stripe takes in thousandths but requires to be a multiple of 10 — the last
/// digit must be zero. We round down so a charge can never exceed what the buyer was shown.
const THREE_DECIMAL: [&str; 5] = ["bhd", "jod", "kwd", "omr", "tnd"];

/// Decimal exponent Stripe expects for `currency`.
fn exponent(currency: &str) -> u32 {
    let c = currency.to_ascii_lowercase();
    if ZERO_DECIMAL.contains(&c.as_str()) {
        0
    } else if THREE_DECIMAL.contains(&c.as_str()) {
        3
    } else {
        2
    }
}

/// Convert a domain `Decimal` amount into the integer minor units Stripe charges in.
///
/// Rejects negatives and anything that does not fit an i64 rather than saturating: a wrong
/// amount here is money, and a loud failure at checkout is recoverable in a way that an
/// overcharge is not.
pub fn to_minor(amount: Decimal, currency: &str) -> Result<i64, ProviderError> {
    if amount.is_sign_negative() {
        return Err(ProviderError::Invalid("amount must be >= 0".into()));
    }
    let exp = exponent(currency);
    let scaled = (amount * Decimal::from(10_i64.pow(exp))).round();
    let minor = scaled
        .to_i64()
        .ok_or_else(|| ProviderError::Invalid("amount out of range".into()))?;
    // Three-decimal currencies must end in 0; round down so we never charge more than shown.
    Ok(if exp == 3 { minor - minor % 10 } else { minor })
}

/// The inverse of [`to_minor`]: a provider-reported amount back into the domain's decimal.
pub fn from_minor(minor: i64, currency: &str) -> Decimal {
    Decimal::from(minor) / Decimal::from(10_i64.pow(exponent(currency)))
}

/// Stripe's documented per-currency minimum charge, in minor units, for the currencies we
/// actually price in. A charge below the minimum is rejected by Stripe with an opaque error;
/// catching it here lets checkout say something a buyer can act on.
pub fn minimum_charge(currency: &str) -> i64 {
    match currency.to_ascii_lowercase().as_str() {
        "usd" | "eur" | "gbp" | "chf" | "cad" | "aud" | "nzd" | "sgd" => 50,
        "uah" => 2000, // ≈ 20 UAH; Stripe quotes UAH minimums well above the 0.50 tier
        "pln" => 200,
        "huf" => 17500,
        "czk" => 1500,
        "sek" | "nok" | "dkk" => 300,
        "ron" => 200,
        "jpy" => 50,
        _ => 50,
    }
}

/// One `application/x-www-form-urlencoded` body. Stripe's API does not take JSON.
#[derive(Default)]
pub struct Form(Vec<(String, String)>);

impl Form {
    pub fn new() -> Self {
        Form(Vec::new())
    }
    pub fn put(mut self, key: &str, value: impl Into<String>) -> Self {
        self.0.push((key.to_string(), value.into()));
        self
    }
    /// Sorted so a body is byte-identical for identical inputs — otherwise an idempotency
    /// retry can look like a different request to Stripe.
    pub fn encode(mut self) -> String {
        self.0.sort();
        self.0
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }
    pub fn pairs(&self) -> &[(String, String)] {
        &self.0
    }
}

/// Percent-encode for `application/x-www-form-urlencoded`, space as `+`.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build the Checkout Session body. Pure, so the exact field names Stripe expects are
/// asserted in a unit test instead of discovered in production.
pub fn checkout_session_form(req: &ChargeReq, minor: i64, fee_minor: i64) -> Form {
    let currency = req.currency.to_ascii_lowercase();
    let title = if req.description.trim().is_empty() {
        "Квиток".to_string()
    } else {
        req.description.clone()
    };
    let mut f = Form::new()
        .put("mode", "payment")
        .put("client_reference_id", req.order_id.to_string())
        .put("metadata[order_id]", req.order_id.to_string())
        // Session metadata is NOT copied to the PaymentIntent or the Charge. Every later
        // event — refund, dispute — arrives on those objects, so without this line none of
        // them can ever be traced back to an order and the whole terminal branch is dead.
        .put("payment_intent_data[metadata][order_id]", req.order_id.to_string())
        .put("line_items[0][quantity]", "1")
        .put("line_items[0][price_data][currency]", &currency)
        .put("line_items[0][price_data][unit_amount]", minor.to_string())
        .put("line_items[0][price_data][product_data][name]", title);
    // Stripe requires both URLs; a missing return_url would otherwise fail deep inside the
    // API with a message the buyer sees as "щось пішло не так".
    let base = req.return_url.clone().unwrap_or_default();
    if !base.is_empty() {
        f = f
            .put("success_url", format!("{base}?order={}&paid=1", req.order_id))
            .put("cancel_url", format!("{base}?order={}&paid=0", req.order_id));
    }
    // The fee only exists on a direct charge: with no connected account the money is already
    // ours, and Stripe rejects an application fee that has no destination.
    if req.connected_account.is_some() && fee_minor > 0 {
        f = f.put("payment_intent_data[application_fee_amount]", fee_minor.to_string());
    }
    f
}

// ------------------------------------------------------------------ webhook signatures

/// One parsed `Stripe-Signature` header: the timestamp it was signed at and every `v1`
/// signature it carries (Stripe sends more than one while a secret is being rotated).
struct SigHeader {
    timestamp: i64,
    v1: Vec<String>,
}

fn parse_signature_header(raw: &str) -> Option<SigHeader> {
    let mut timestamp = None;
    let mut v1 = Vec::new();
    for part in raw.split(',') {
        let (k, v) = part.trim().split_once('=')?;
        match k {
            "t" => timestamp = v.parse::<i64>().ok(),
            "v1" => v1.push(v.to_string()),
            _ => {}
        }
    }
    let timestamp = timestamp?;
    if v1.is_empty() {
        return None;
    }
    Some(SigHeader { timestamp, v1 })
}

/// The signature Stripe computes: HMAC-SHA256 over `"{timestamp}.{body}"`, hex encoded.
pub fn sign_payload(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Verify a `Stripe-Signature` header against the raw body.
///
/// `now` is injected so the tolerance window is testable without sleeping, and so a clock
/// skew bug shows up as a failing test rather than as webhooks that stop arriving.
pub fn verify_signature(secret: &str, header: &str, body: &[u8], now: i64) -> Result<(), ProviderError> {
    let parsed = parse_signature_header(header).ok_or_else(|| ProviderError::Invalid("malformed signature".into()))?;
    if (now - parsed.timestamp).abs() > SIGNATURE_TOLERANCE_SECS {
        return Err(ProviderError::Invalid("signature timestamp outside tolerance".into()));
    }
    let expected = sign_payload(secret, parsed.timestamp, body);
    // Constant-time compare against every offered signature so a rotation window works and
    // no byte-by-byte timing signal leaks.
    let ok = parsed.v1.iter().any(|candidate| {
        hex::decode(candidate).is_ok_and(|bytes| {
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
            mac.update(parsed.timestamp.to_string().as_bytes());
            mac.update(b".");
            mac.update(body);
            mac.verify_slice(&bytes).is_ok()
        })
    });
    if ok {
        Ok(())
    } else {
        let _ = expected;
        Err(ProviderError::Invalid("signature mismatch".into()))
    }
}

/// The slice of a Stripe event we act on. Everything else is acknowledged and dropped.
#[derive(Deserialize)]
struct StripeEvent {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: StripeEventData,
}

#[derive(Deserialize)]
struct StripeEventData {
    object: serde_json::Value,
}

/// Map an authenticated Stripe event onto our normalized action.
///
/// The order id comes from OUR metadata rather than any Stripe identifier: it is the only
/// value we control end to end, and it survives Stripe replacing the session with a
/// PaymentIntent between `created` and `completed`.
pub fn map_event(raw: &[u8]) -> Result<InboundWebhook, ProviderError> {
    let ev: StripeEvent = serde_json::from_slice(raw).map_err(|_| ProviderError::Invalid("invalid json".into()))?;
    let obj = &ev.data.object;
    let order_ref = obj["metadata"]["order_id"]
        .as_str()
        .map(str::to_string)
        // A refund arrives on the charge, whose metadata carries the PaymentIntent's copy.
        .or_else(|| obj["payment_intent"].as_str().map(str::to_string));
    let event = match ev.kind.as_str() {
        // Payment finished and the session is paid. `payment_status` guards against acting on
        // a completed session whose asynchronous payment method has not actually cleared.
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            let paid = obj["payment_status"].as_str().unwrap_or("paid") == "paid";
            match (paid, order_ref.clone()) {
                (true, Some(order_ref)) => WebhookEvent::Paid {
                    order_ref,
                    provider_ref: obj["payment_intent"].as_str().map(str::to_string),
                    // The session id is what checkout stored on the order; the consumer binds
                    // on it, not on metadata.
                    charge_ref: obj["id"].as_str().map(str::to_string),
                    amount_minor: obj["amount_total"].as_i64(),
                    currency: obj["currency"].as_str().map(str::to_string),
                },
                _ => WebhookEvent::Ignored,
            }
        }
        // Arrives on the Charge. `amount_refunded` is cumulative, so a second partial refund
        // reports the running total, not the increment.
        "charge.refunded" => match order_ref.clone() {
            Some(order_ref) => WebhookEvent::Refunded {
                order_ref,
                refunded_minor: obj["amount_refunded"].as_i64(),
                total_minor: obj["amount"].as_i64(),
            },
            None => WebhookEvent::Ignored,
        },
        // Arrives on the Refund object, which knows its own amount but not the charge total;
        // the consumer treats "total unknown" as "not proven full".
        "refund.succeeded" => match order_ref.clone() {
            Some(order_ref) => WebhookEvent::Refunded {
                order_ref,
                refunded_minor: obj["amount"].as_i64(),
                total_minor: None,
            },
            None => WebhookEvent::Ignored,
        },
        // A chargeback. Terminal for the money and not reversible by us — the escrow row
        // must freeze rather than continue toward a payout of funds already clawed back.
        "charge.dispute.created" => match order_ref.clone() {
            Some(order_ref) => WebhookEvent::Disputed { order_ref },
            None => WebhookEvent::Ignored,
        },
        // Payout lifecycle: until one of these lands, our `payouts.status` is a claim about
        // what we asked for, not a fact about what reached the organiser's bank.
        "payout.paid" => match obj["metadata"]["payout_id"].as_str() {
            Some(payout_ref) => WebhookEvent::PayoutPaid {
                payout_ref: payout_ref.to_string(),
            },
            None => WebhookEvent::Ignored,
        },
        "payout.failed" => match obj["metadata"]["payout_id"].as_str() {
            Some(payout_ref) => WebhookEvent::PayoutFailed {
                payout_ref: payout_ref.to_string(),
                reason: obj["failure_message"].as_str().unwrap_or("payout failed").to_string(),
            },
            None => WebhookEvent::Ignored,
        },
        // How we learn an organiser finished onboarding — or broke it — without polling.
        // `requirements_due` drives the "доонбордьтесь" banner in the organiser cabinet.
        "account.updated" => match obj["id"].as_str() {
            Some(account_ref) => WebhookEvent::AccountUpdated {
                account_ref: account_ref.to_string(),
                charges_enabled: obj["charges_enabled"].as_bool().unwrap_or(false),
                payouts_enabled: obj["payouts_enabled"].as_bool().unwrap_or(false),
                requirements_due: obj["requirements"]["currently_due"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty()),
            },
            None => WebhookEvent::Ignored,
        },
        _ => WebhookEvent::Ignored,
    };
    Ok(InboundWebhook {
        id: Some(ev.id),
        kind: ev.kind,
        event,
    })
}

// ------------------------------------------------------------------ the provider

pub struct StripeProvider {
    secret_key: String,
    webhook_secret: Option<String>,
    base_url: String,
    http: reqwest::Client,
}

impl StripeProvider {
    pub fn new(secret_key: impl Into<String>, webhook_secret: Option<String>, base_url: impl Into<String>) -> Self {
        StripeProvider {
            secret_key: secret_key.into(),
            webhook_secret,
            base_url: base_url.into(),
            http: http_client().clone(),
        }
    }

    /// POST a form body and return the parsed JSON, or a provider error carrying Stripe's own
    /// message. Stripe's error text is safe to surface: it never contains our key.
    ///
    /// Retries — bounded, backed off, jittered — happen ONLY when an idempotency key is
    /// present: repeating a request without one could charge or refund twice. 429 and 5xx are
    /// retried; a 4xx is Stripe telling us we are wrong, and asking again will not change that.
    async fn post(
        &self,
        path: &str,
        form: Form,
        connected_account: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<serde_json::Value, ProviderError> {
        let body = form.encode();
        let attempts = if idempotency_key.is_some() { MAX_ATTEMPTS } else { 1 };
        let mut last = ProviderError::Transport("no attempt made".into());
        for attempt in 0..attempts {
            if attempt > 0 {
                // 200ms, 400ms … plus up to 100ms of jitter so a burst of retries spreads out.
                let jitter = u64::from(std::process::id() ^ attempt) % 100;
                tokio::time::sleep(std::time::Duration::from_millis(200 * (1 << (attempt - 1)) + jitter)).await;
            }
            let mut rb = self
                .http
                .post(format!("{}{}", self.base_url, path))
                .bearer_auth(&self.secret_key)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body.clone());
            if let Some(acct) = connected_account {
                rb = rb.header("Stripe-Account", acct);
            }
            if let Some(key) = idempotency_key {
                rb = rb.header("Idempotency-Key", key);
            }
            let resp = match rb.send().await {
                Ok(r) => r,
                Err(e) => {
                    last = ProviderError::Transport(format!("stripe request failed: {e}"));
                    continue;
                }
            };
            let status = resp.status();
            // Cap what we are willing to parse: Stripe's own responses are a few KB, and an
            // unbounded read on a compromised or misrouted host is a memory sink.
            let bytes = match resp.bytes().await {
                Ok(b) if b.len() <= MAX_RESPONSE_BYTES => b,
                Ok(_) => return Err(ProviderError::Transport("stripe response too large".into())),
                Err(e) => {
                    last = ProviderError::Transport(format!("stripe response unreadable: {e}"));
                    continue;
                }
            };
            let parsed: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => return Err(ProviderError::Transport(format!("stripe response not json: {e}"))),
            };
            if status.is_success() {
                return Ok(parsed);
            }
            let msg = parsed["error"]["message"]
                .as_str()
                .unwrap_or("stripe rejected the request")
                .to_string();
            if status.as_u16() == 429 || status.is_server_error() {
                last = ProviderError::Upstream(msg);
                continue;
            }
            return Err(ProviderError::Upstream(msg));
        }
        Err(last)
    }

    /// `GET /v1/account` — used by the admin "перевірити зв'язок" button. Returns the account
    /// id so the operator can see WHICH account the key belongs to, not merely that it works.
    pub async fn check_connection(&self) -> Result<String, ProviderError> {
        let resp = self
            .http
            .get(format!("{}/v1/account", self.base_url))
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(format!("stripe request failed: {e}")))?;
        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Transport(format!("stripe response not json: {e}")))?;
        if !status.is_success() {
            let msg = body["error"]["message"]
                .as_str()
                .unwrap_or("ключ відхилено")
                .to_string();
            return Err(ProviderError::Upstream(msg));
        }
        Ok(body["id"].as_str().unwrap_or_default().to_string())
    }
}

#[async_trait::async_trait]
impl PaymentProvider for StripeProvider {
    fn name(&self) -> &'static str {
        "stripe"
    }
    fn moves_real_money(&self) -> bool {
        true
    }

    async fn create_charge(&self, req: ChargeReq) -> Result<ChargeResult, ProviderError> {
        let minor = to_minor(req.amount, &req.currency)?;
        let min = minimum_charge(&req.currency);
        if minor < min {
            return Err(ProviderError::Invalid(format!(
                "сума менша за мінімальну для {} ({} мінімум)",
                req.currency.to_uppercase(),
                min
            )));
        }
        let fee_minor = to_minor(req.application_fee, &req.currency)?;
        if fee_minor >= minor {
            return Err(ProviderError::Invalid(
                "комісія не може дорівнювати сумі або перевищувати її".into(),
            ));
        }
        let form = checkout_session_form(&req, minor, fee_minor);
        // The order id as idempotency key: a retried checkout returns the SAME session
        // instead of creating a second one the buyer could also pay.
        let body = self
            .post(
                "/v1/checkout/sessions",
                form,
                req.connected_account.as_deref(),
                Some(&req.order_id.to_string()),
            )
            .await?;
        let id = body["id"]
            .as_str()
            .ok_or_else(|| ProviderError::Upstream("stripe returned no session id".into()))?;
        Ok(ChargeResult {
            provider_ref: id.to_string(),
            checkout_url: body["url"].as_str().map(str::to_string),
            status: ChargeStatus::Pending,
        })
    }

    async fn transfer_payout(&self, req: PayoutReq) -> Result<TransferResult, ProviderError> {
        let minor = to_minor(req.amount, &req.currency)?;
        let form = Form::new()
            .put("amount", minor.to_string())
            .put("currency", req.currency.to_ascii_lowercase())
            .put("metadata[payout_id]", req.payout_id.to_string());
        let body = self
            .post(
                "/v1/payouts",
                form,
                req.connected_account.as_deref(),
                Some(&req.payout_id.to_string()),
            )
            .await?;
        let id = body["id"]
            .as_str()
            .ok_or_else(|| ProviderError::Upstream("stripe returned no payout id".into()))?;
        // Stripe reports `paid` only once the bank confirms; anything else is still moving.
        let status = match body["status"].as_str().unwrap_or("pending") {
            "paid" => PayoutStatus::Paid,
            "failed" | "canceled" => PayoutStatus::Failed,
            _ => PayoutStatus::Processing,
        };
        Ok(TransferResult {
            provider_ref: id.to_string(),
            status,
        })
    }

    async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError> {
        let minor = to_minor(req.amount, &req.currency)?;
        let mut form = Form::new().put("payment_intent", req.provider_ref.clone());
        // Omitting `amount` tells Stripe to refund everything; sending it enables a partial.
        if minor > 0 {
            form = form.put("amount", minor.to_string());
        }
        if let Some(reason) = req.reason.as_deref().filter(|r| !r.trim().is_empty()) {
            form = form.put("metadata[reason]", reason);
        }
        // The application fee is returned with the refund, otherwise a full refund leaves our
        // commission taken from money the buyer no longer paid.
        if req.refund_application_fee {
            form = form.put("refund_application_fee", "true");
        }
        let body = self
            .post(
                "/v1/refunds",
                form,
                req.connected_account.as_deref(),
                req.idempotency_key.as_deref(),
            )
            .await?;
        let id = body["id"]
            .as_str()
            .ok_or_else(|| ProviderError::Upstream("stripe returned no refund id".into()))?;
        let status = match body["status"].as_str().unwrap_or("pending") {
            "failed" | "canceled" => RefundStatus::Failed,
            _ => RefundStatus::Succeeded,
        };
        Ok(RefundResult {
            provider_ref: id.to_string(),
            status,
        })
    }

    fn verify_webhook(&self, headers: &HeaderMap, raw: &[u8]) -> Result<InboundWebhook, ProviderError> {
        let secret = self
            .webhook_secret
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ProviderError::Invalid("webhook secret not configured".into()))?;
        let header = headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ProviderError::Invalid("missing stripe-signature".into()))?;
        verify_signature(secret, header, raw, chrono::Utc::now().timestamp())?;
        map_event(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromStr;
    use uuid::Uuid;

    fn req(amount: &str, currency: &str) -> ChargeReq {
        ChargeReq {
            order_id: Uuid::nil(),
            amount: Decimal::from_str(amount).unwrap(),
            currency: currency.into(),
            return_url: Some("https://underground.test".into()),
            description: "Нічний рейв".into(),
            connected_account: None,
            application_fee: Decimal::ZERO,
        }
    }

    // ---------------------------------------------------------------- minor units

    #[test]
    fn two_decimal_currency_scales_by_100() {
        assert_eq!(to_minor(Decimal::from_str("250.00").unwrap(), "UAH").unwrap(), 25_000);
        assert_eq!(to_minor(Decimal::from_str("19.99").unwrap(), "eur").unwrap(), 1_999);
    }

    // The bug this guards is a factor of 100 that reconciles to a plausible number: ¥1000 sent
    // as 1000 minor units charges ¥1000, but sent as 100000 charges ¥100 000.
    #[test]
    fn zero_decimal_currency_is_not_scaled() {
        assert_eq!(to_minor(Decimal::from_str("1000").unwrap(), "JPY").unwrap(), 1_000);
        assert_eq!(to_minor(Decimal::from_str("50000").unwrap(), "krw").unwrap(), 50_000);
    }

    #[test]
    fn three_decimal_currency_rounds_down_to_a_multiple_of_ten() {
        // 9.999 KWD → 9999 thousandths → Stripe requires the last digit to be 0.
        assert_eq!(to_minor(Decimal::from_str("9.999").unwrap(), "KWD").unwrap(), 9_990);
        assert_eq!(to_minor(Decimal::from_str("1.000").unwrap(), "bhd").unwrap(), 1_000);
    }

    #[test]
    fn negative_amount_is_rejected_not_clamped() {
        let err = to_minor(Decimal::from_str("-1").unwrap(), "UAH").unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(m) if m.contains(">= 0")));
    }

    #[test]
    fn amount_beyond_i64_is_rejected() {
        let huge = Decimal::from_str("99999999999999999999").unwrap();
        assert!(matches!(to_minor(huge, "UAH"), Err(ProviderError::Invalid(_))));
    }

    #[test]
    fn currency_case_does_not_change_the_scale() {
        assert_eq!(
            to_minor(Decimal::ONE, "JPY").unwrap(),
            to_minor(Decimal::ONE, "jpy").unwrap()
        );
    }

    #[test]
    fn minimum_charge_is_known_for_our_currencies_and_defaulted_otherwise() {
        assert_eq!(minimum_charge("UAH"), 2000);
        assert_eq!(minimum_charge("usd"), 50);
        assert_eq!(minimum_charge("huf"), 17500);
        assert_eq!(minimum_charge("zzz"), 50, "unknown currency falls back, never panics");
    }

    // ---------------------------------------------------------------- form building

    #[test]
    fn checkout_form_carries_the_fields_stripe_requires() {
        let f = checkout_session_form(&req("250.00", "UAH"), 25_000, 0);
        let pairs: std::collections::HashMap<_, _> = f.pairs().iter().cloned().collect();
        assert_eq!(pairs["mode"], "payment");
        assert_eq!(pairs["line_items[0][price_data][currency]"], "uah");
        assert_eq!(pairs["line_items[0][price_data][unit_amount]"], "25000");
        assert_eq!(pairs["line_items[0][price_data][product_data][name]"], "Нічний рейв");
        assert_eq!(pairs["metadata[order_id]"], Uuid::nil().to_string());
        assert!(pairs["success_url"].contains("paid=1"));
        assert!(pairs["cancel_url"].contains("paid=0"));
    }

    #[test]
    fn application_fee_is_sent_only_on_a_direct_charge() {
        let mut r = req("250.00", "UAH");
        let without: std::collections::HashMap<_, _> = checkout_session_form(&r, 25_000, 2_500)
            .pairs()
            .iter()
            .cloned()
            .collect();
        assert!(
            !without.contains_key("payment_intent_data[application_fee_amount]"),
            "no connected account = the money is already ours; Stripe rejects a fee with no destination"
        );
        r.connected_account = Some("acct_123".into());
        let with: std::collections::HashMap<_, _> = checkout_session_form(&r, 25_000, 2_500)
            .pairs()
            .iter()
            .cloned()
            .collect();
        assert_eq!(with["payment_intent_data[application_fee_amount]"], "2500");
    }

    #[test]
    fn empty_description_falls_back_to_a_readable_line_item() {
        let mut r = req("250.00", "UAH");
        r.description = "   ".into();
        let pairs: std::collections::HashMap<_, _> =
            checkout_session_form(&r, 25_000, 0).pairs().iter().cloned().collect();
        assert_eq!(pairs["line_items[0][price_data][product_data][name]"], "Квиток");
    }

    #[test]
    fn form_encoding_is_stable_and_escapes() {
        let a = Form::new().put("b", "2").put("a", "hello world&x=1").encode();
        let b = Form::new().put("a", "hello world&x=1").put("b", "2").encode();
        assert_eq!(a, b, "sorted so an idempotent retry is byte-identical");
        assert!(a.contains("hello+world%26x%3D1"));
    }

    #[test]
    fn checkout_form_omits_urls_when_no_return_url_is_configured() {
        let mut r = req("250.00", "UAH");
        r.return_url = None;
        let pairs: std::collections::HashMap<_, _> =
            checkout_session_form(&r, 25_000, 0).pairs().iter().cloned().collect();
        assert!(!pairs.contains_key("success_url"));
    }

    // ---------------------------------------------------------------- signatures

    fn signed_header(secret: &str, ts: i64, body: &[u8]) -> String {
        format!("t={},v1={}", ts, sign_payload(secret, ts, body))
    }

    #[test]
    fn valid_signature_passes() {
        let body = br#"{"id":"evt_1"}"#;
        let h = signed_header("whsec_x", 1_000, body);
        assert!(verify_signature("whsec_x", &h, body, 1_000).is_ok());
    }

    #[test]
    fn signature_from_a_different_secret_is_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let h = signed_header("whsec_attacker", 1_000, body);
        assert!(verify_signature("whsec_real", &h, body, 1_000).is_err());
    }

    #[test]
    fn a_tampered_body_is_rejected() {
        let h = signed_header("whsec_x", 1_000, b"{\"amount\":100}");
        assert!(verify_signature("whsec_x", &h, b"{\"amount\":100000}", 1_000).is_err());
    }

    // Without the window a captured delivery replays forever: the signature itself never expires.
    #[test]
    fn a_replay_outside_the_tolerance_window_is_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let h = signed_header("whsec_x", 1_000, body);
        assert!(verify_signature("whsec_x", &h, body, 1_000 + SIGNATURE_TOLERANCE_SECS + 1).is_err());
        assert!(verify_signature("whsec_x", &h, body, 1_000 + SIGNATURE_TOLERANCE_SECS).is_ok());
        // Skew in the other direction too — a clock ahead of Stripe's must not accept forever.
        assert!(verify_signature("whsec_x", &h, body, 1_000 - SIGNATURE_TOLERANCE_SECS - 1).is_err());
    }

    #[test]
    fn malformed_signature_headers_are_rejected() {
        let body = b"{}";
        for bad in ["", "garbage", "t=abc,v1=00", "v1=00", "t=1000"] {
            assert!(
                verify_signature("whsec_x", bad, body, 1_000).is_err(),
                "must reject {bad:?}"
            );
        }
    }

    #[test]
    fn any_offered_signature_may_match_during_secret_rotation() {
        let body = br#"{"id":"evt_1"}"#;
        let good = sign_payload("whsec_new", 1_000, body);
        let header = format!("t=1000,v1=deadbeef,v1={good}");
        assert!(verify_signature("whsec_new", &header, body, 1_000).is_ok());
    }

    #[test]
    fn non_hex_signature_candidate_does_not_panic() {
        let body = b"{}";
        assert!(verify_signature("whsec_x", "t=1000,v1=zzzz", body, 1_000).is_err());
    }

    // ---------------------------------------------------------------- event mapping

    fn event(kind: &str, object: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"id":"evt_1","type":kind,"data":{"object":object}})).unwrap()
    }

    #[test]
    fn completed_session_maps_to_paid_with_the_refundable_handle() {
        let raw = event(
            "checkout.session.completed",
            serde_json::json!({"payment_status":"paid","payment_intent":"pi_1","metadata":{"order_id":"o-1"}}),
        );
        let hook = map_event(&raw).unwrap();
        assert_eq!(hook.id.as_deref(), Some("evt_1"));
        assert_eq!(
            hook.event,
            WebhookEvent::Paid {
                order_ref: "o-1".into(),
                provider_ref: Some("pi_1".into()),
                charge_ref: None,
                amount_minor: None,
                currency: None,
            }
        );
    }

    /// The consumer binds on the session id and the paid amount; both must come through.
    #[test]
    fn completed_session_carries_the_binding_fields() {
        let raw = event(
            "checkout.session.completed",
            serde_json::json!({"id":"cs_9","payment_status":"paid","payment_intent":"pi_9","amount_total":25000,
                               "currency":"uah","metadata":{"order_id":"o-9"}}),
        );
        assert_eq!(
            map_event(&raw).unwrap().event,
            WebhookEvent::Paid {
                order_ref: "o-9".into(),
                provider_ref: Some("pi_9".into()),
                charge_ref: Some("cs_9".into()),
                amount_minor: Some(25_000),
                currency: Some("uah".into()),
            }
        );
    }

    // A session can complete while an asynchronous method is still clearing. Minting there
    // would hand out a ticket for money that may never arrive.
    #[test]
    fn completed_session_that_is_not_paid_is_ignored() {
        let raw = event(
            "checkout.session.completed",
            serde_json::json!({"payment_status":"unpaid","metadata":{"order_id":"o-1"}}),
        );
        assert_eq!(map_event(&raw).unwrap().event, WebhookEvent::Ignored);
    }

    #[test]
    fn refund_and_dispute_map_to_terminal_events() {
        let refunded = event("charge.refunded", serde_json::json!({"metadata":{"order_id":"o-2"}}));
        assert_eq!(
            map_event(&refunded).unwrap().event,
            WebhookEvent::Refunded {
                order_ref: "o-2".into(),
                refunded_minor: None,
                total_minor: None,
            }
        );
        let disputed = event(
            "charge.dispute.created",
            serde_json::json!({"metadata":{"order_id":"o-3"}}),
        );
        assert_eq!(
            map_event(&disputed).unwrap().event,
            WebhookEvent::Disputed {
                order_ref: "o-3".into()
            }
        );
    }

    #[test]
    fn payout_lifecycle_maps_to_payout_events() {
        let paid = event("payout.paid", serde_json::json!({"metadata":{"payout_id":"p-1"}}));
        assert_eq!(
            map_event(&paid).unwrap().event,
            WebhookEvent::PayoutPaid {
                payout_ref: "p-1".into()
            }
        );
        let failed = event(
            "payout.failed",
            serde_json::json!({"metadata":{"payout_id":"p-2"},"failure_message":"account closed"}),
        );
        assert_eq!(
            map_event(&failed).unwrap().event,
            WebhookEvent::PayoutFailed {
                payout_ref: "p-2".into(),
                reason: "account closed".into()
            }
        );
    }

    #[test]
    fn account_updated_carries_the_capability_flags() {
        let raw = event(
            "account.updated",
            serde_json::json!({"id":"acct_1","charges_enabled":true,"payouts_enabled":false,
                               "requirements":{"currently_due":["individual.verification.document"]}}),
        );
        assert_eq!(
            map_event(&raw).unwrap().event,
            WebhookEvent::AccountUpdated {
                account_ref: "acct_1".into(),
                charges_enabled: true,
                payouts_enabled: false,
                requirements_due: true,
            }
        );
    }

    #[test]
    fn events_without_our_reference_are_ignored_rather_than_guessed() {
        for kind in [
            "checkout.session.completed",
            "charge.refunded",
            "charge.dispute.created",
            "payout.paid",
            "payout.failed",
            "account.updated",
        ] {
            let raw = event(kind, serde_json::json!({"payment_status":"paid"}));
            assert_eq!(map_event(&raw).unwrap().event, WebhookEvent::Ignored, "{kind}");
        }
    }

    #[test]
    fn unknown_event_types_are_acknowledged_not_rejected() {
        let raw = event("invoice.created", serde_json::json!({}));
        let hook = map_event(&raw).unwrap();
        assert_eq!(hook.event, WebhookEvent::Ignored);
        assert_eq!(hook.kind, "invoice.created");
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(matches!(map_event(b"not json"), Err(ProviderError::Invalid(_))));
    }

    #[test]
    fn refund_falls_back_to_the_payment_intent_when_metadata_is_absent() {
        let raw = event("charge.refunded", serde_json::json!({"payment_intent":"pi_9"}));
        assert_eq!(
            map_event(&raw).unwrap().event,
            WebhookEvent::Refunded {
                order_ref: "pi_9".into(),
                refunded_minor: None,
                total_minor: None,
            }
        );
    }

    // ---------------------------------------------------------------- provider surface

    #[test]
    fn provider_identifies_itself_as_real_money() {
        let p = StripeProvider::new("sk_test_x", None, DEFAULT_BASE_URL);
        assert_eq!(p.name(), "stripe");
        assert!(p.moves_real_money(), "the gate that keeps demo flows off a live key");
    }

    #[test]
    fn webhook_without_a_configured_secret_is_refused() {
        let p = StripeProvider::new("sk_test_x", None, DEFAULT_BASE_URL);
        let err = p.verify_webhook(&HeaderMap::new(), b"{}").unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(m) if m.contains("webhook secret")));
    }

    #[test]
    fn webhook_without_a_signature_header_is_refused() {
        let p = StripeProvider::new("sk_test_x", Some("whsec_x".into()), DEFAULT_BASE_URL);
        let err = p.verify_webhook(&HeaderMap::new(), b"{}").unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(m) if m.contains("stripe-signature")));
    }

    #[tokio::test]
    async fn charge_below_the_currency_minimum_never_reaches_the_network() {
        let p = StripeProvider::new("sk_test_x", None, "http://127.0.0.1:1");
        let err = p.create_charge(req("1.00", "UAH")).await.unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(m) if m.contains("мінімальну")));
    }

    #[tokio::test]
    async fn a_fee_that_swallows_the_whole_charge_is_refused() {
        let mut r = req("250.00", "UAH");
        r.connected_account = Some("acct_1".into());
        r.application_fee = Decimal::from_str("250.00").unwrap();
        let p = StripeProvider::new("sk_test_x", None, "http://127.0.0.1:1");
        let err = p.create_charge(r).await.unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(m) if m.contains("комісія")));
    }

    #[test]
    fn base_url_defaults_to_stripe_and_trims_an_override() {
        std::env::remove_var("STRIPE_API_BASE");
        assert_eq!(base_url().unwrap(), DEFAULT_BASE_URL);
        std::env::set_var("STRIPE_API_BASE", "http://localhost:1234/");
        assert_eq!(
            base_url().unwrap(),
            "http://localhost:1234",
            "loopback http is allowed for a mock"
        );
        std::env::set_var("STRIPE_API_BASE", "https://stripe-mock.internal");
        assert_eq!(base_url().unwrap(), "https://stripe-mock.internal");
        std::env::set_var("STRIPE_API_BASE", "   ");
        assert_eq!(base_url().unwrap(), DEFAULT_BASE_URL, "blank override falls back");
        // [SEC] The secret key rides as a bearer header. Plain http to a non-loopback host
        // would send it in clear; a typo in a compose file must be refused, not obeyed.
        std::env::set_var("STRIPE_API_BASE", "http://internal-proxy:8080");
        assert!(matches!(base_url(), Err(ProviderError::Invalid(_))));
        std::env::remove_var("STRIPE_API_BASE");
    }

    #[test]
    fn from_minor_inverts_to_minor_for_each_exponent() {
        assert_eq!(from_minor(25_000, "uah"), Decimal::from_str("250.00").unwrap());
        assert_eq!(from_minor(1_000, "jpy"), Decimal::from(1_000));
        assert_eq!(from_minor(9_990, "kwd"), Decimal::from_str("9.990").unwrap());
    }
}
