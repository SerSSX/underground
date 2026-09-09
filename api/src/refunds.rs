//! The refund executor (PAY-11).
//!
//! Before this module a "refund" was a status flip: `admin::escrow_action("refund")` and
//! `tickets::return_ticket` moved rows around and never spoke to the payment provider, so the
//! buyer's money stayed exactly where it was. Here a refund becomes a JOB, and one worker drains
//! the queue against the provider with retries, so every source of a refund — an admin, a buyer
//! self-returning, a cancelled event, a payment that could not be honoured (PAY-3), a refund
//! policy (REF-*) — goes through one door and gets the same guarantees.
//!
//! Two properties carry the whole module:
//!
//! * **Never return more than was paid.** Checked under `FOR UPDATE` on the order against the
//!   settled total plus everything still in flight, and backed by a database CHECK that catches
//!   the writer who forgets. Money leaving twice is the one failure with no clean recovery.
//! * **Never report money moved that has not.** For a DIRECT charge, a provider whose organiser
//!   has too small a balance answers "pending", not an error. A job stays open until the
//!   provider says the money actually left, and a webhook is what closes it.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::provider::{PaymentProvider, RefundReq, RefundStatus};

/// How many times a job may fail transiently before a human is asked to look. Beyond this the
/// row does not disappear — it becomes `needs_attention`, which is visible to the organiser and
/// to the platform admin.
const MAX_ATTEMPTS: i32 = 6;

/// Jobs claimed per pass. Small enough that one slow provider cannot hold the loop for minutes,
/// large enough that a 200-ticket cancellation drains in a few passes.
const BATCH: i64 = 20;

/// How long a job may sit `sent` before the worker tries it again. The retry carries the SAME
/// idempotency key, so the provider returns the refund it already made rather than a second one —
/// which is exactly how a crash between "provider said yes" and "we wrote it down" is recovered.
const LEASE_MINUTES: i64 = 15;

/// After this, a retry is no longer safe: the provider only honours an idempotency key for about
/// a day, and past that the same key is treated as a NEW request — a second refund, money out
/// twice. Such a job stops and waits for a human instead.
const KEY_VALID_HOURS: i64 = 23;

/// Advisory-lock key for the whole executor. Two replicas draining the same queue would issue
/// two refunds for one order between the claim and the write.
const LOCK_KEY: i64 = 0x5245_4655_4E44; // "REFUND"

/// Queue a refund for `order_id`. THE single entry point for every source of a refund.
///
/// Returns `true` when a new job was queued, `false` when this refund is already on record —
/// callers key their own idempotency off that rather than counting rows themselves.
///
/// Refuses, rather than truncates, a refund that would take the order past what the buyer paid:
/// a caller asking for too much is a bug, and silently returning less would hide it.
pub async fn enqueue(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    amount: Decimal,
    reason: &str,
) -> Result<bool, AppError> {
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("сума повернення має бути більшою за нуль".into()));
    }
    // FOR UPDATE: the ceiling is read-then-written, so two concurrent enqueues for one order
    // must not both see room for the last hryvnia.
    let row: Option<(Decimal, Decimal, String, String)> =
        sqlx::query_as("SELECT amount, refunded_amount, currency, status FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (paid, settled, currency, order_status) = row.ok_or(AppError::NotFound)?;
    // Money can only come back if it went out. An abandoned checkout, a failed charge or a free
    // order has nothing to return, and queueing one would send the provider a refund for a
    // payment that never existed (OR-23).
    if !matches!(order_status.as_str(), "paid" | "refund_due" | "refunded") {
        return Err(AppError::Conflict(
            "за цим замовленням не було оплати — повертати нічого".into(),
        ));
    }

    // Everything still in flight counts against the ceiling too. A job that has been sent to the
    // provider but not yet confirmed is money already committed; treating it as free room is how
    // an order gets refunded twice.
    let in_flight: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0) FROM refund_jobs \
         WHERE order_id = $1 AND status IN ('queued', 'sent')",
    )
    .bind(order_id)
    .fetch_one(&mut *conn)
    .await?;

    if settled + in_flight + amount > paid {
        return Err(AppError::Conflict(
            "сума повернення перевищує сплачену за це замовлення".into(),
        ));
    }

    // `seq` is per order, so a partial today and the rest tomorrow are two jobs, while the SAME
    // refund queued twice collides on (order_id, seq) and is refused by the index.
    let next_seq: i32 = sqlx::query_scalar("SELECT COALESCE(max(seq), 0) + 1 FROM refund_jobs WHERE order_id = $1")
        .bind(order_id)
        .fetch_one(&mut *conn)
        .await?;
    let inserted = sqlx::query(
        "INSERT INTO refund_jobs (order_id, seq, amount, currency, reason) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (order_id, seq) DO NOTHING",
    )
    .bind(order_id)
    .bind(next_seq)
    .bind(amount)
    .bind(&currency)
    .bind(reason)
    .execute(&mut *conn)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

/// Whether a failed attempt is worth repeating. Kept as its own function so the decision is
/// readable and testable apart from the SQL around it: retrying a refund the provider has
/// permanently refused only delays the moment a person is told, and retrying a network blip is
/// free.
pub fn worth_retrying(err: &crate::provider::ProviderError) -> bool {
    use crate::provider::ProviderError as E;
    match err {
        // Our own request was malformed, or the provider refused on the merits (already
        // refunded, more than the remaining amount, charge disputed). Repeating it is pointless.
        E::Invalid(_) | E::Upstream(_) => false,
        // The network, or the provider being briefly unavailable.
        E::Transport(_) => true,
    }
}

/// Provider messages are shown to the organiser. Keep them short and single-line: the raw text
/// is the provider's own and carries no key of ours, but an unbounded blob in a UI is how a
/// stack trace ends up in front of a customer.
fn sanitize(msg: &str) -> String {
    let flat: String = msg.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(300).collect()
}

/// Run one pass of the queue. Returns `(settled, failed)` for the log.
///
/// Takes a session-level advisory lock so two API replicas never drain the queue at once. The
/// lock is tried, not waited on: a second replica simply does nothing this tick.
pub async fn run_once(db: &PgPool, provider: &dyn PaymentProvider) -> Result<(u64, u64), AppError> {
    let mut lock = db.acquire().await?;
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(LOCK_KEY)
        .fetch_one(&mut *lock)
        .await?;
    if !got {
        return Ok((0, 0));
    }
    let result = drain(db, provider).await;
    // Release even when the pass failed, or this replica holds the queue until it restarts.
    let _ = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_KEY)
        .fetch_one(&mut *lock)
        .await;
    result
}

async fn drain(db: &PgPool, provider: &dyn PaymentProvider) -> Result<(u64, u64), AppError> {
    // Claim by flipping to 'sent' in the same statement that selects: a job is either ours or
    // another pass's, never both. SKIP LOCKED so one slow row cannot block the batch.
    // Everything the decision needs comes from the ORDER, not from today's settings: which
    // provider actually took this money, what handle it left behind, and how much of the order
    // is still unreturned.
    type Job = (
        Uuid,
        Uuid,
        i32,
        Decimal,
        String,
        Option<String>,
        Option<String>,
        String,
        Decimal,
    );
    let jobs: Vec<Job> = sqlx::query_as(
        "UPDATE refund_jobs j SET status = 'sent', attempts = attempts + 1, \
            next_attempt_at = now() + make_interval(mins => $2) \
         FROM orders o WHERE o.id = j.order_id AND j.id IN ( \
             SELECT id FROM refund_jobs \
             WHERE status IN ('queued', 'sent') AND next_attempt_at <= now() \
               AND created_at > now() - make_interval(hours => $3) \
             ORDER BY next_attempt_at LIMIT $1 FOR UPDATE SKIP LOCKED) \
         RETURNING j.id, j.order_id, j.seq, j.amount, j.currency, o.provider_ref, \
             o.connected_account, o.provider, o.amount",
    )
    .bind(BATCH)
    .bind(LEASE_MINUTES as i32)
    .bind(KEY_VALID_HOURS as i32)
    .fetch_all(db)
    .await?;

    // A job older than the provider's idempotency window can no longer be retried safely, so it
    // stops here and is shown to a person rather than risking a second refund.
    let stale = sqlx::query(
        "UPDATE refund_jobs SET status = 'needs_attention', \
            last_error = 'повернення не підтверджене провайдером — потрібна ручна перевірка' \
         WHERE status IN ('queued', 'sent') AND created_at <= now() - make_interval(hours => $1)",
    )
    .bind(KEY_VALID_HOURS as i32)
    .execute(db)
    .await?;
    if stale.rows_affected() > 0 {
        tracing::warn!(
            count = stale.rows_affected(),
            "refunds: jobs aged out of the retry window"
        );
    }

    let (mut settled, mut failed) = (0u64, 0u64);
    for (job_id, order_id, seq, amount, currency, provider_ref, connected_account, order_provider, order_total) in jobs
    {
        // The money was taken by THIS provider; refunding through whatever is configured today
        // would send a live charge's refund to a provider that never saw it — and the test
        // provider would cheerfully report success while nothing moved (OR-18).
        if order_provider != provider.name() {
            mark_attention(
                db,
                job_id,
                "замовлення оплачене іншим провайдером — потрібне ручне повернення",
            )
            .await?;
            failed += 1;
            continue;
        }
        // A checkout SESSION is not a payment. Refunding against `cs_...` fails at the provider
        // with a confusing error; it means the payment webhook never swapped in the real handle,
        // which is a state a person must look at (OR-19).
        let Some(charge_ref) = provider_ref.filter(|r| !r.is_empty() && !r.starts_with("cs_")) else {
            mark_attention(db, job_id, "замовлення не має підтвердженого платежу у провайдера").await?;
            failed += 1;
            continue;
        };
        // Is this the slice that finishes the order off? Decided from the queue rather than from
        // the settled total, so it is the same answer whichever order the jobs happen to run in
        // — two halves claimed in one batch would otherwise BOTH look non-final, and the
        // commission would never come back at all.
        let committed: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(sum(amount), 0) FROM refund_jobs \
             WHERE order_id = $1 AND seq <= $2 AND status IN ('queued', 'sent', 'done')",
        )
        .bind(order_id)
        .bind(seq)
        .fetch_one(db)
        .await?;
        let closing = committed >= order_total;

        let res = provider
            .refund(RefundReq {
                provider_ref: charge_ref,
                amount,
                currency,
                reason: None,
                connected_account,
                // Our commission comes back only on the slice that FINISHES the refund.
                // Sending it on every partial would return the fee two or three times over,
                // out of the platform's own balance (OR-20).
                refund_application_fee: closing,
                // The SAME key for every attempt at this job, so a retry after a timeout returns
                // the original refund instead of issuing a second one.
                idempotency_key: Some(format!("refund:{order_id}:{seq}")),
            })
            .await;

        match res {
            Ok(r) if r.status == RefundStatus::Succeeded => {
                settle(db, job_id, order_id, &r.provider_ref).await?;
                settled += 1;
            }
            Ok(r) => {
                // Accepted but not moved (a direct charge against too small a balance), or
                // already failed outright at the provider.
                if r.status == RefundStatus::Failed {
                    mark_attention(db, job_id, "провайдер відхилив повернення").await?;
                    failed += 1;
                } else {
                    sqlx::query("UPDATE refund_jobs SET provider_ref = $2 WHERE id = $1")
                        .bind(job_id)
                        .bind(&r.provider_ref)
                        .execute(db)
                        .await?;
                    tracing::info!(%order_id, "refunds: accepted by the provider, awaiting settlement");
                }
            }
            Err(e) if worth_retrying(&e) => {
                reschedule(db, job_id, &sanitize(&e.to_string())).await?;
            }
            Err(e) => {
                mark_attention(db, job_id, &sanitize(&e.to_string())).await?;
                failed += 1;
            }
        }
    }
    Ok((settled, failed))
}

/// The money moved: credit the order's settled total and close the job. One transaction, because
/// a job marked done without its money recorded would free room under the ceiling for a second
/// refund of the same amount.
async fn settle(db: &PgPool, job_id: Uuid, order_id: Uuid, provider_ref: &str) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    settle_on(&mut tx, job_id, order_id, provider_ref).await?;
    tx.commit().await?;
    Ok(())
}

/// The settlement writes themselves, on a caller-owned connection. Split out so a webhook can
/// record "this refund settled" in the SAME transaction that marks the delivery as seen: commit
/// the dedup row first and a failure here would burn the event id, and the provider's retry —
/// the only other chance to learn the money moved — would be dropped as a duplicate.
async fn settle_on(
    conn: &mut sqlx::PgConnection,
    job_id: Uuid,
    order_id: Uuid,
    provider_ref: &str,
) -> Result<(), AppError> {
    let done =
        sqlx::query("UPDATE refund_jobs SET status = 'done', provider_ref = $2 WHERE id = $1 AND status <> 'done'")
            .bind(job_id)
            .bind(provider_ref)
            .execute(&mut *conn)
            .await?;
    // Keyed on rows_affected: a webhook and the worker can both learn the same refund settled,
    // and only the first may add it to the total.
    if done.rows_affected() > 0 {
        // DERIVED from the settled jobs rather than incremented, and never allowed to go down.
        // Three different places record refunds — this executor, the provider's full-refund
        // notification, and its partial notification — and a blind `+= amount` double-counts the
        // moment two of them describe the same money. A sum over `done` jobs is idempotent
        // however many times it runs, and GREATEST keeps the provider's own cumulative figure
        // from being lowered by ours (OR-10, OR-11).
        sqlx::query(
            "UPDATE orders SET refunded_amount = GREATEST(refunded_amount, \
                    (SELECT COALESCE(sum(amount), 0) FROM refund_jobs WHERE order_id = $1 AND status = 'done')) \
             WHERE id = $1",
        )
        .bind(order_id)
        .execute(&mut *conn)
        .await?;
        // Close the escrow only once the ORDER is whole again: a partial leaves the rest still
        // owed to the organiser, so it must not read as fully returned.
        sqlx::query("UPDATE orders SET escrow_status = 'refunded' WHERE id = $1 AND refunded_amount >= amount")
            .bind(order_id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Back off with jitter, or hand the job to a human once it has had enough tries.
async fn reschedule(db: &PgPool, job_id: Uuid, why: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE refund_jobs SET status = CASE WHEN attempts >= $2 THEN 'needs_attention' ELSE 'queued' END, \
            last_error = $3, \
            next_attempt_at = now() + (interval '1 minute' * power(4, least(attempts, 5)) * (0.75 + random() * 0.5)) \
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(MAX_ATTEMPTS)
    .bind(why)
    .execute(db)
    .await?;
    Ok(())
}

async fn mark_attention(db: &PgPool, job_id: Uuid, why: &str) -> Result<(), AppError> {
    let mut conn = db.acquire().await?;
    mark_attention_on(&mut conn, job_id, why).await
}

async fn mark_attention_on(conn: &mut sqlx::PgConnection, job_id: Uuid, why: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE refund_jobs SET status = 'needs_attention', last_error = $2 WHERE id = $1")
        .bind(job_id)
        .bind(why)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// A provider webhook resolved a refund we had sent. This is the ONLY way a `pending` refund
/// becomes settled — the API answer alone never proves the money left.
pub async fn resolve_from_webhook(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    refund_ref: &str,
    status: &str,
    failure_reason: Option<&str>,
) -> Result<(), AppError> {
    let job: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM refund_jobs \
         WHERE order_id = $1 AND (provider_ref = $2 OR provider_ref IS NULL) AND status = 'sent' \
         ORDER BY seq LIMIT 1",
    )
    .bind(order_id)
    .bind(refund_ref)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((job_id,)) = job else {
        return Ok(());
    };
    match status {
        "succeeded" => settle_on(&mut *conn, job_id, order_id, refund_ref).await?,
        "failed" | "canceled" => {
            mark_attention_on(
                &mut *conn,
                job_id,
                failure_reason.map(sanitize).unwrap_or_default().as_str(),
            )
            .await?
        }
        // Still in flight (`pending`, `requires_action`): leave it alone and wait.
        _ => {}
    }
    Ok(())
}

/// What the organiser's cabinet shows while a cancellation drains.
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct Progress {
    pub total: i64,
    pub done: i64,
    pub in_flight: i64,
    pub needs_attention: i64,
    pub amount_refunded: Decimal,
}

/// `GET /api/events/:id/refund-progress` — organiser (owner) or platform admin.
pub async fn event_progress(db: &PgPool, event_id: Uuid) -> Result<Progress, AppError> {
    let p: Progress = sqlx::query_as(
        "SELECT count(*) AS total, \
            count(*) FILTER (WHERE j.status = 'done') AS done, \
            count(*) FILTER (WHERE j.status IN ('queued','sent')) AS in_flight, \
            count(*) FILTER (WHERE j.status = 'needs_attention') AS needs_attention, \
            COALESCE(sum(j.amount) FILTER (WHERE j.status = 'done'), 0) AS amount_refunded \
         FROM refund_jobs j JOIN orders o ON o.id = j.order_id WHERE o.event_id = $1",
    )
    .bind(event_id)
    .fetch_one(db)
    .await?;
    Ok(p)
}
