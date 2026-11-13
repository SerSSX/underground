#!/usr/bin/env node
// TASK-305 regression check: chat send on an iPhone (WebKit with touch) and the organiser's public page.
//
// What it does, against a deployed web app (DEV; never prod, it creates an account):
//   1. registers a throwaway account (random e-mail under example.com, random password, both kept in memory only);
//   2. finds an event with an organiser card (or uses EVENT=<event id>) and taps its «Написати» / Message link;
//   3. taps the message field and checks the caret is at the end of the «Щодо «…»: » prefill, types, taps Send;
//   4. asserts the bubble appears, the field is empty and still focused (keyboard stays up);
//   5. sends a second message with the keyboard's Enter (Send) key and asserts that bubble too;
//   6. back on the event page, taps the organiser card and asserts /u/<organiser id> shows the organiser's name;
//   7. opens /u/<unknown id> and asserts the friendly "not found";
//   8. always deletes the throwaway account at the end (the API also deletes its messages both ways).
// Product telemetry (/api/telemetry) is blocked so the run does not count as a user.
//
// Playwright is not a dependency of this repo. It is loaded from PLAYWRIGHT_PATH, by default the copy installed in
// msgview: /Users/serhii/Projects/msgview/node_modules/playwright (its WebKit build must be installed:
// `npx playwright install webkit` in that project).
//
// Usage:
//   node web/e2e/chat-webkit.mjs https://underground-dev.65-108-87-247.sslip.io
//   BASE=https://underground-dev.65-108-87-247.sslip.io EVENT=<event uuid> node web/e2e/chat-webkit.mjs
//   HEADED=1 shows the browser. Exit code 0 = all checks passed, 1 = a check failed (a screenshot path is printed).
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PW = process.env.PLAYWRIGHT_PATH || "/Users/serhii/Projects/msgview/node_modules/playwright";
const BASE = (process.argv[2] || process.env.BASE || "").replace(/\/+$/, "");
if (!/^https?:\/\//.test(BASE)) {
  console.error("usage: node web/e2e/chat-webkit.mjs <base url>   (or BASE=<base url>)");
  process.exit(2);
}
if (/^https:\/\/(www\.)?underground\./.test(BASE) && !/-dev\./.test(BASE)) {
  console.error("refusing to run against what looks like prod: this script creates an account");
  process.exit(2);
}
const { webkit, devices } = createRequire(import.meta.url)(PW);

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
const stamp = Date.now().toString(36);
const email = `e2e-chat-${stamp}@example.com`;
const password = `E2e-${stamp}-${Math.random().toString(36).slice(2, 10)}`;
const results = [];
let failed = false;

function check(name, ok, detail = "") {
  results.push(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failed = true;
  return ok;
}

const browser = await webkit.launch({ headless: !process.env.HEADED });
const context = await browser.newContext({ ...devices["iPhone 13"], locale: "uk-UA" });
await context.route("**/api/telemetry", (r) => r.abort());
const page = await context.newPage();
const pageErrors = [];
page.on("pageerror", (e) => pageErrors.push(String(e)));
// The API origin, learnt from the app's own requests (it may differ from the web origin).
let apiOrigin = null;
page.on("request", (r) => {
  const u = new URL(r.url());
  if (!apiOrigin && u.pathname.startsWith("/api/")) apiOrigin = u.origin;
});
let registered = false;

try {
  // 1. Register.
  await page.goto(`${BASE}/register?next=${encodeURIComponent("/account")}`, { waitUntil: "networkidle" });
  await page.fill("#name", `E2E Chat ${stamp}`);
  await page.fill("#email", email);
  await page.fill("#pw", password);
  await page.tap("form button.btn.block");
  await page.waitForURL(/\/account/, { timeout: 15000 });
  registered = await page.evaluate(() => !!localStorage.getItem("ug_web_token"));
  if (!check("register a throwaway account", registered)) throw new Error("no token after sign-up");

  // 2. An event with an organiser to write to.
  let eventId = process.env.EVENT || null;
  if (!eventId) {
    await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
    const hrefs = [...new Set(await page.$$eval('a[href^="/events/"]', (as) => as.map((a) => a.getAttribute("href"))))].slice(0, 12);
    for (const h of hrefs) {
      await page.goto(`${BASE}${h}`, { waitUntil: "networkidle" });
      if (await page.locator('.organizer-card a[href^="/inbox?to="]').count()) { eventId = h.match(UUID)?.[0] ?? null; break; }
    }
  }
  if (!check("find an event with an organiser card", !!eventId)) throw new Error("no event with an organiser on the home page; pass EVENT=<id>");
  await page.goto(`${BASE}/events/${eventId}`, { waitUntil: "networkidle" });
  const organizerName = (await page.locator(".organizer-card .organizer-link .strong").first().innerText()).trim();
  const msgLink = page.locator('.organizer-card a[href^="/inbox?to="]');
  const organizerId = (await msgLink.getAttribute("href"))?.match(UUID)?.[0];
  await msgLink.tap();

  // 3. Field, prefill and caret.
  const field = page.locator("#msg");
  await field.waitFor({ state: "visible", timeout: 15000 });
  await page.waitForFunction(() => (document.querySelector("#msg")?.value ?? "").length > 0, null, { timeout: 8000 }).catch(() => {});
  await field.tap();
  await page.waitForTimeout(150);
  const caret = await field.evaluate((el) => ({ value: el.value, start: el.selectionStart, end: el.selectionEnd }));
  if (caret.value) check("caret at the end of the prefill on tap", caret.start === caret.value.length && caret.end === caret.value.length, `${caret.start}/${caret.value.length}`);
  else check("event prefill present", false, "the field was empty; the «Щодо «…»: » text did not arrive");

  // 4. Tap Send.
  const first = `e2e tap ${stamp}`;
  await page.keyboard.type(first);
  const typed = await field.inputValue();
  check("typing appends to the prefill", typed.endsWith(first), JSON.stringify(typed));
  await page.locator(".chat-compose button[type=submit]").tap();
  const bubble1 = await page.locator(".bubble.mine", { hasText: first }).first().waitFor({ timeout: 10000 }).then(() => true, () => false);
  check("tapping Send shows the message bubble", bubble1);
  check("the field is empty after Send", (await field.inputValue()) === "");
  check("the field keeps focus after Send", await page.evaluate(() => document.activeElement?.id === "msg"));

  // 5. Keyboard Send (Enter).
  const second = `e2e enter ${stamp}`;
  await page.keyboard.type(second);
  await page.keyboard.press("Enter");
  const bubble2 = await page.locator(".bubble.mine", { hasText: second }).first().waitFor({ timeout: 10000 }).then(() => true, () => false);
  check("Enter (keyboard Send) shows the message bubble", bubble2);

  // The chat header links to the same person.
  const headHref = await page.locator(".chat-head a.chat-peer").getAttribute("href").catch(() => null);
  check("chat header links to the profile", headHref === `/u/${organizerId}`, String(headHref));

  // 6. Organiser card → public page.
  await page.goto(`${BASE}/events/${eventId}`, { waitUntil: "networkidle" });
  await page.locator(".organizer-card .organizer-link").tap();
  await page.waitForURL(new RegExp(`/u/${organizerId}$`), { timeout: 10000 }).catch(() => {});
  check("organiser card opens /u/<organiser id>", page.url().endsWith(`/u/${organizerId}`), page.url());
  const shownName = await page.locator("h1.profile-name").innerText({ timeout: 10000 }).catch(() => "");
  check("profile page shows the organiser's name", shownName.trim().startsWith(organizerName), JSON.stringify(shownName));
  check("profile page offers «Написати»", (await page.locator(`a[href="/inbox?to=${organizerId}"]`).count()) > 0);

  // 7. Unknown profile.
  await page.goto(`${BASE}/u/00000000-0000-4000-8000-000000000000`, { waitUntil: "networkidle" });
  check("unknown profile shows the friendly not-found", (await page.locator(".empty h1").count()) === 1);

  check("no uncaught page errors", pageErrors.length === 0, pageErrors.join(" | ").slice(0, 300));
} catch (e) {
  check("run finished", false, String(e?.message ?? e).slice(0, 300));
} finally {
  if (failed) {
    const shot = join(tmpdir(), `chat-webkit-${stamp}.png`);
    await page.screenshot({ path: shot, fullPage: true }).catch(() => {});
    results.push(`screenshot: ${shot}`);
  }
  // 8. Remove the throwaway account (and with it, its messages).
  if (registered) {
    const status = await page.evaluate(async ({ api, pw }) => {
      const token = localStorage.getItem("ug_web_token");
      if (!api || !token) return "no api origin or token";
      const r = await fetch(`${api}/api/me`, {
        method: "DELETE",
        headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
        body: JSON.stringify({ password: pw }),
      });
      return String(r.status);
    }, { api: apiOrigin, pw: password }).catch((e) => String(e));
    results.push(`cleanup: delete account ${email} -> ${status}`);
  }
  await browser.close();
  console.log(results.join("\n"));
  process.exit(failed ? 1 : 0);
}
