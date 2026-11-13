// Where an alert in the inbox leads (TASK-305). The server sends `link` for a place offer (`/g/<token>`) and a new
// booking (`/organizer/<id>`); anything that is not a plain path on this site is ignored, so an alert can never send
// the user to another host, a scheme or a query. Without a usable link the alert opens its event, if it has one.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const PLAIN_PATH = /^\/[A-Za-z0-9_-][A-Za-z0-9/_-]{0,200}$/;

export function alertHref(n) {
  const link = n?.link;
  if (typeof link === "string" && PLAIN_PATH.test(link) && !link.includes("//")) return link;
  const ev = n?.event_id;
  return typeof ev === "string" && UUID.test(ev) ? `/events/${ev}` : null;
}
