"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { getToken, getMe, patchMe, getPrefs, putPrefs, exportMyData, deleteMyAccount, uploadImage, errorMessage, EVENT_CATEGORIES, type UserDto } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import { categoryLabel, initials } from "../../lib/ui";
import CityInput from "../components/CityInput";
import Modal from "../components/Modal";
import LanguagePicker from "../components/LanguagePicker";
import { Toast, useToast } from "../components/Toast";
import { LayoutGrid, Ticket, LogOut, Download, Trash2, Camera, Check, Globe, ChevronRight } from "../components/Icon";

export default function AccountPage() {
  const router = useRouter();
  const { lang } = useLang();
  const { logout, refresh } = useSession();
  const [me, setMe] = useState<UserDto | null>(null);
  const [name, setName] = useState("");
  const [dob, setDob] = useState("");
  const [city, setCity] = useState("");
  // The city's country, from prefs or a picked suggestion; typing a name drops it.
  const [cc, setCc] = useState<string | null>(null);
  const [interests, setInterests] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [del, setDel] = useState(false);
  const [noPw, setNoPw] = useState(false);
  const [proof, setProof] = useState("");
  const [toast, showToast] = useToast();

  useEffect(() => {
    if (!getToken()) {
      router.replace("/login?next=/account");
      return;
    }
    getMe().then((u) => { setMe(u); setName(u.name); setDob(u.dob ?? ""); }).catch((e) => setError(errorMessage(e)));
    getPrefs().then((p) => { setCity(p.city ?? ""); setCc(p.country_code ?? null); setInterests(p.categories); }).catch(() => {});
  }, [router]);

  async function run(key: string, fn: () => Promise<void>, ok?: string) {
    setBusy(key);
    setError(null);
    try {
      await fn();
      if (ok) showToast(ok);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  const saveProfile = () => run("profile", async () => {
    const u = await patchMe({ name: name.trim(), ...(dob ? { dob } : {}) });
    setMe(u);
    refresh();
  }, tr(lang, "Профіль збережено", "Profile saved"));

  const saveAvatar = (file: File) => run("avatar", async () => {
    const url = await uploadImage(file);
    setMe(await patchMe({ avatar_url: url }));
    refresh();
  }, tr(lang, "Фото оновлено", "Photo updated"));

  const savePrefs = () => run("prefs", async () => { await putPrefs(interests, city.trim() || null, cc); }, tr(lang, "Інтереси збережено", "Interests saved"));

  const doExport = () => run("export", async () => {
    const data = await exportMyData();
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
    a.download = "underground-my-data.json";
    a.click();
  });

  const doDelete = () => run("delete", async () => {
    await deleteMyAccount(noPw ? { confirm: "DELETE" } : { password: proof });
    logout();
    router.push("/");
  });

  if (!me) return <main className="container narrow">{error ? <div className="alert error" style={{ marginTop: 24 }}>{error}</div> : <div className="skeleton" style={{ height: 240, marginTop: 24 }} />}</main>;

  const toggle = (c: string) => setInterests((s) => (s.includes(c) ? s.filter((x) => x !== c) : [...s, c]));

  return (
    <main className="container narrow">
      <div className="card" style={{ display: "flex", alignItems: "center", gap: 18, marginTop: 8, padding: 22 }}>
        <label style={{ position: "relative", cursor: "pointer" }} aria-label={tr(lang, "Змінити фото", "Change photo")}>
          <span className="avatar lg" style={{ width: 72, height: 72, fontSize: 26 }}>{me.avatar_url ? <img src={me.avatar_url} alt="" /> : initials(me.name)}</span>
          <span className="round-btn" style={{ position: "absolute", right: -6, bottom: -6, width: 30, height: 30, background: "var(--surface-3)" }}><Camera className="ico sm" /></span>
          <input type="file" accept="image/jpeg,image/png,image/webp" className="sr-only" onChange={(e) => { const f = e.target.files?.[0]; if (f) void saveAvatar(f); }} />
        </label>
        <div className="grow">
          <h1 style={{ fontSize: 24 }}>{me.name}</h1>
          <div className="muted small">{me.email}</div>
        </div>
      </div>

      <div className="stack" style={{ marginTop: 16 }}>
        <Link href="/me/tickets" className="card tight row gap"><Ticket className="ico" /><span className="grow strong">{tr(lang, "Мої квитки", "My tickets")}</span><ChevronRight className="ico sm subtle" /></Link>
        <Link href="/organizer" className="card tight row gap"><LayoutGrid className="ico" /><span className="grow strong">{tr(lang, "Мої події", "My events")}</span><ChevronRight className="ico sm subtle" /></Link>
      </div>

      {error && <div className="alert error" style={{ marginTop: 16 }}>{error}</div>}

      <section className="card section">
        <h2 style={{ fontSize: 17, marginBottom: 14 }}>{tr(lang, "Особисті дані", "Personal details")}</h2>
        <div className="form">
          <div className="field">
            <label htmlFor="acc-name">{tr(lang, "Імʼя", "Name")}</label>
            <input id="acc-name" className="input" value={name} maxLength={80} onChange={(e) => setName(e.target.value)} />
            <span className="hint">{tr(lang, "Організатор бачить лише ваше імʼя і кількість місць.", "Organisers see only your name and number of places.")}</span>
          </div>
          <div className="field">
            <label htmlFor="acc-dob">{tr(lang, "Дата народження", "Date of birth")}</label>
            <input id="acc-dob" className="input" type="date" value={dob} max={new Date().toISOString().slice(0, 10)} onChange={(e) => setDob(e.target.value)} style={{ maxWidth: 240 }} />
            <span className="hint">{tr(lang, "Потрібна лише для подій з віковим обмеженням. Нікому не показується.", "Only for age-restricted events. Never shown to anyone.")}</span>
          </div>
          <button className="btn" style={{ alignSelf: "flex-start" }} onClick={saveProfile} disabled={busy === "profile" || !name.trim()}>{tr(lang, "Зберегти", "Save")}</button>
        </div>
      </section>

      <section className="card section">
        <h2 style={{ fontSize: 17 }}>{tr(lang, "Інтереси й місто", "Interests and city")}</h2>
        <p className="muted small" style={{ marginTop: 4, marginBottom: 14 }}>{tr(lang, "За ними складається добірка «Для тебе».", "These shape your For You picks.")}</p>
        <div className="field" style={{ maxWidth: 320, marginBottom: 14 }}>
          <label htmlFor="acc-city">{tr(lang, "Місто", "City")}</label>
          <CityInput id="acc-city" value={city} onChange={(v) => { setCity(v); setCc(null); }} onPick={(p, name) => { setCity(name); setCc(p.country_code); }} />
        </div>
        <div className="chips">
          {EVENT_CATEGORIES.map((c) => {
            const on = interests.includes(c);
            return <button key={c} type="button" className={`chip ${on ? "active" : ""}`} aria-pressed={on} onClick={() => toggle(c)}><Check className="ico" style={{ opacity: on ? 1 : 0 }} aria-hidden />{categoryLabel(c, lang)}</button>;
          })}
        </div>
        <button className="btn secondary" style={{ marginTop: 16 }} onClick={savePrefs} disabled={busy === "prefs"}>{tr(lang, "Зберегти інтереси", "Save interests")}</button>
      </section>

      <section className="card section">
        <div className="row spread wrap" style={{ gap: 12 }}>
          <div className="row gap"><Globe className="ico" /><span className="strong">{tr(lang, "Мова інтерфейсу", "Language")}</span></div>
          <LanguagePicker />
        </div>
      </section>

      <section className="card section">
        <h2 style={{ fontSize: 17, marginBottom: 12 }}>{tr(lang, "Дані й акаунт", "Data and account")}</h2>
        <div className="row gap wrap">
          <button className="btn ghost sm" onClick={doExport} disabled={busy === "export"}><Download className="ico sm" />{tr(lang, "Завантажити мої дані", "Download my data")}</button>
          <button className="btn ghost sm" onClick={() => { logout(); router.push("/"); }}><LogOut className="ico sm" />{tr(lang, "Вийти", "Log out")}</button>
          <button className="btn danger-ghost sm" onClick={() => { setProof(""); setDel(true); }}><Trash2 className="ico sm" />{tr(lang, "Видалити акаунт", "Delete account")}</button>
        </div>
      </section>

      {del && (
        <Modal title={tr(lang, "Видалити акаунт?", "Delete account?")} onClose={() => busy !== "delete" && setDel(false)}>
          <form className="form" onSubmit={(e) => { e.preventDefault(); void doDelete(); }}>
            <p className="muted">{tr(lang, "Ваші дані буде знеособлено, активні бронювання скасовано. Відновити акаунт не вийде.", "Your data is anonymised and active bookings cancelled. This cannot be undone.")}</p>
            {noPw ? (
              <div className="field">
                <label htmlFor="del-confirm">{tr(lang, "Введіть DELETE, щоб підтвердити", "Type DELETE to confirm")}</label>
                <input id="del-confirm" className="input" value={proof} onChange={(e) => setProof(e.target.value)} autoComplete="off" />
              </div>
            ) : (
              <div className="field">
                <label htmlFor="del-pw">{tr(lang, "Ваш пароль", "Your password")}</label>
                <input id="del-pw" className="input" type="password" value={proof} onChange={(e) => setProof(e.target.value)} autoComplete="current-password" />
              </div>
            )}
            <button type="button" className="btn quiet sm" style={{ alignSelf: "flex-start", marginLeft: -12 }} onClick={() => { setNoPw((v) => !v); setProof(""); }}>
              {noPw ? tr(lang, "У мене є пароль", "I have a password") : tr(lang, "Я входжу без пароля", "I sign in without a password")}
            </button>
            {error && <div className="alert error">{error}</div>}
            <button className="btn danger block" disabled={busy === "delete" || (noPw ? proof !== "DELETE" : !proof)}>{tr(lang, "Видалити назавжди", "Delete forever")}</button>
          </form>
        </Modal>
      )}
      <Toast msg={toast} />
    </main>
  );
}
