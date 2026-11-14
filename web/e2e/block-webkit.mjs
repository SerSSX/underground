#!/usr/bin/env node
// TASK-306 browser check: blocking another person, on an iPhone (WebKit with touch), with two throwaway accounts.
//
// Against a deployed web app (DEV; never prod, it creates accounts):
//   1. registers A and B (random e-mails under example.com, random passwords, kept in memory only);
//   2. A opens B's page, taps «Заблокувати», accepts the confirmation (whose text must not promise B won't notice),
//      and sees «Ви заблокували цю людину» with «Розблокувати» instead of «Написати»;
//   3. B writes to A: the message does not go, the neutral «Цій людині зараз не можна написати.» shows and the text
//      is given back in the field;
//   4. A opens the chat with B: no message field, an unblock button instead;
//   5. A's /account lists B under «Заблоковані»; unblocking there empties the list;
//   6. A blocks again from B's page, then unblocks from the closed chat: the field is back and A's message goes;
//   7. no uncaught page errors; both accounts are always deleted at the end.
// Product telemetry (/api/telemetry) is blocked so the run does not count as a user.
//
// Playwright is loaded from PLAYWRIGHT_PATH (default: the copy in msgview), as in e2e/chat-webkit.mjs.
// Usage: node web/e2e/block-webkit.mjs https://underground-dev.65-108-87-247.sslip.io   (HEADED=1 shows the browser)
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PW = process.env.PLAYWRIGHT_PATH || "/Users/serhii/Projects/msgview/node_modules/playwright";
const BASE = (process.argv[2] || process.env.BASE || "").replace(/\/+$/, "");
if (!/^https?:\/\//.test(BASE)) {
  console.error("usage: node web/e2e/block-webkit.mjs <base url>   (or BASE=<base url>)");
  process.exit(2);
}
if (/^https:\/\/(www\.)?underground\./.test(BASE) && !/-dev\./.test(BASE)) {
  console.error("refusing to run against what looks like prod: this script creates accounts");
  process.exit(2);
}
const { webkit, devices } = createRequire(import.meta.url)(PW);

const stamp = Date.now().toString(36);
const results = [];
let failed = false;
function check(name, ok, detail = "") {
  results.push(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failed = true;
  return ok;
}

const browser = await webkit.launch({ headless: !process.env.HEADED });
const pageErrors = [];

/** One person: its own browser context (own localStorage), a page, and what cleanup needs. */
async function person(tag) {
  const context = await browser.newContext({ ...devices["iPhone 13"], locale: "uk-UA" });
  await context.route("**/api/telemetry", (r) => r.abort());
  const page = await context.newPage();
  page.on("pageerror", (e) => pageErrors.push(`${tag}: ${e}`));
  const p = {
    tag, page, context, registered: false, apiOrigin: null, id: null,
    name: `E2E Block ${tag} ${stamp}`,
    email: `e2e-block-${tag.toLowerCase()}-${stamp}@example.com`,
    password: `E2e-${stamp}-${Math.random().toString(36).slice(2, 10)}`,
  };
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (!p.apiOrigin && u.pathname.startsWith("/api/")) p.apiOrigin = u.origin;
  });
  await page.goto(`${BASE}/register?next=${encodeURIComponent("/account")}`, { waitUntil: "domcontentloaded" });
  await page.fill("#name", p.name);
  await page.fill("#email", p.email);
  await page.fill("#pw", p.password);
  await page.tap("form button.btn.block");
  await page.waitForURL(/\/account/, { timeout: 15000 });
  p.registered = await page.evaluate(() => !!localStorage.getItem("ug_web_token"));
  if (!p.registered) throw new Error(`no token after sign-up of ${tag}`);
  p.id = await page.evaluate(async (api) => {
    const r = await fetch(`${api}/api/me`, { headers: { authorization: `Bearer ${localStorage.getItem("ug_web_token")}` } });
    return (await r.json()).id;
  }, p.apiOrigin);
  return p;
}

async function openProfile(p, other) {
  await p.page.goto(`${BASE}/u/${other.id}`, { waitUntil: "domcontentloaded" });
  await p.page.waitForSelector(".profile-name", { timeout: 15000 });
  await p.page.waitForTimeout(800); // the signed-in re-read brings is_blocked
}

let A = null;
let B = null;
try {
  A = await person("A");
  B = await person("B");
  check("register two throwaway accounts", !!A.id && !!B.id);

  // 2. A blocks B from B's page.
  let dialogText = "";
  A.page.once("dialog", (d) => { dialogText = d.message(); void d.accept(); });
  await openProfile(A, B);
  await A.page.getByRole("button", { name: "Заблокувати" }).tap();
  await A.page.getByText("Ви заблокували цю людину").waitFor({ timeout: 10000 });
  check("block asks for confirmation", dialogText.includes(B.name), dialogText.slice(0, 80));
  check("confirmation does not promise they won't notice", dialogText.includes("Ми їй про це не повідомимо") && !dialogText.includes("не дізнається"));
  check("blocked profile shows Unblock", await A.page.getByRole("button", { name: "Розблокувати" }).isVisible());
  check("blocked profile hides Message", (await A.page.getByRole("link", { name: "Написати" }).count()) === 0);

  // 3. B tries to write to A.
  await B.page.goto(`${BASE}/inbox?to=${A.id}`, { waitUntil: "domcontentloaded" });
  const field = B.page.locator("#msg");
  await field.waitFor({ state: "visible", timeout: 15000 });
  const text = `e2e blocked ${stamp}`;
  await field.tap();
  await B.page.keyboard.type(text);
  await B.page.locator(".chat-compose button[type=submit]").tap();
  const neutral = await B.page.getByText("Цій людині зараз не можна написати.").waitFor({ timeout: 10000 }).then(() => true, () => false);
  check("B sees the neutral can't-message text", neutral);
  check("B's text is given back in the field", (await field.inputValue()).includes(text));
  check("no bubble for the refused message", (await B.page.locator(".bubble.mine", { hasText: text }).count()) === 0);

  // 4. A's chat with B is closed.
  await A.page.goto(`${BASE}/inbox?to=${B.id}`, { waitUntil: "domcontentloaded" });
  const closed = await A.page.getByText("Ви заблокували цю людину: повідомлення не надсилаються.").waitFor({ timeout: 15000 }).then(() => true, () => false);
  check("A's chat with B is closed", closed);
  check("closed chat has no message field", (await A.page.locator("#msg").count()) === 0);

  // 5. «Заблоковані» on /account, unblock there.
  await A.page.goto(`${BASE}/account`, { waitUntil: "domcontentloaded" });
  const section = A.page.locator("#blocked");
  await section.getByText(B.name).waitFor({ timeout: 15000 });
  check("account lists B under Blocked", true);
  await section.getByRole("button", { name: "Розблокувати" }).tap();
  const emptied = await section.getByText("Ви нікого не блокували.").waitFor({ timeout: 10000 }).then(() => true, () => false);
  check("unblock from the account list empties it", emptied);

  // 6. Block again, unblock from the closed chat, then write.
  A.page.once("dialog", (d) => void d.accept());
  await openProfile(A, B);
  await A.page.getByRole("button", { name: "Заблокувати" }).tap();
  await A.page.getByText("Ви заблокували цю людину").waitFor({ timeout: 10000 });
  await A.page.goto(`${BASE}/inbox?to=${B.id}`, { waitUntil: "domcontentloaded" });
  await A.page.getByRole("button", { name: "Розблокувати" }).tap();
  const back = await A.page.locator("#msg").waitFor({ state: "visible", timeout: 10000 }).then(() => true, () => false);
  check("unblock from the chat brings the field back", back);
  const hello = `e2e after unblock ${stamp}`;
  await A.page.locator("#msg").tap();
  await A.page.keyboard.type(hello);
  await A.page.locator(".chat-compose button[type=submit]").tap();
  const sent = await A.page.locator(".bubble.mine", { hasText: hello }).first().waitFor({ timeout: 10000 }).then(() => true, () => false);
  check("A's message goes after the unblock", sent);

  check("no uncaught page errors", pageErrors.length === 0, pageErrors.join(" | ").slice(0, 300));
} catch (e) {
  check("run finished", false, String(e?.message ?? e).slice(0, 300));
} finally {
  for (const p of [A, B]) {
    if (!p) continue;
    if (failed) {
      const shot = join(tmpdir(), `block-webkit-${p.tag}-${stamp}.png`);
      await p.page.screenshot({ path: shot, fullPage: true }).catch(() => {});
      results.push(`screenshot ${p.tag}: ${shot}`);
    }
    if (p.registered) {
      const status = await p.page.evaluate(async ({ api, pw }) => {
        const token = localStorage.getItem("ug_web_token");
        if (!api || !token) return "no api origin or token";
        const r = await fetch(`${api}/api/me`, {
          method: "DELETE",
          headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
          body: JSON.stringify({ password: pw }),
        });
        return String(r.status);
      }, { api: p.apiOrigin, pw: p.password }).catch((e) => String(e));
      results.push(`cleanup: delete account ${p.email} -> ${status}`);
    }
  }
  await browser.close();
  console.log(results.join("\n"));
  process.exit(failed ? 1 : 0);
}
