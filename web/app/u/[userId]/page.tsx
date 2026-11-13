import type { Metadata } from "next";
import ProfileClient from "./ProfileClient";
import { isUuid, serverApiBase, type PublicProfile } from "../../../lib/api";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** Server-side profile fetch for the first paint and the title (public endpoint, no auth, so `is_following` is
 *  false here; the client reads it again signed in). Only a UUID ever goes into the API path. */
async function fetchProfile(id: string): Promise<PublicProfile | null> {
  if (!isUuid(id)) return null;
  try {
    const res = await fetch(`${API}/api/users/${id}`, { next: { revalidate: 60 } });
    if (!res.ok) return null;
    return (await res.json()) as PublicProfile;
  } catch {
    return null;
  }
}

/** A person's page is not for search engines (like /account and /inbox): it names a private person, and the
 *  organiser chose to be shown to visitors of their events, not to be indexed. Links still get a preview. */
export async function generateMetadata({ params }: { params: Promise<{ userId: string }> }): Promise<Metadata> {
  const { userId } = await params;
  const p = await fetchProfile(userId);
  if (!p) return { title: { absolute: "Underground" }, robots: { index: false, follow: false } };
  const description = (p.bio ?? "").slice(0, 180) || undefined;
  const url = `${SITE}/u/${p.id}`;
  return {
    title: p.name,
    description,
    robots: { index: false, follow: true },
    alternates: { canonical: url },
    openGraph: { title: p.name, description, url, type: "profile", siteName: "Underground", images: p.avatar_url ? [{ url: p.avatar_url }] : [] },
  };
}

export default async function UserPage({ params }: { params: Promise<{ userId: string }> }) {
  const { userId } = await params;
  const profile = await fetchProfile(userId);
  return <ProfileClient userId={userId} initialProfile={profile} />;
}
