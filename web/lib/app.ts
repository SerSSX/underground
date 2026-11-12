// The Android build served from our own server (no Google Play yet). The deploy puts the APK and a
// small manifest under /downloads/android/; when the manifest is missing, no banner is shown.
export interface AndroidRelease {
  version: string;
  version_code?: number;
  /** Path or URL of the APK. */
  url: string;
  size_bytes?: number;
  min_android?: string;
  released_at?: string;
  sha256?: string;
}

let cached: Promise<AndroidRelease | null> | null = null;

export function getAndroidRelease(): Promise<AndroidRelease | null> {
  if (!cached) {
    cached = fetch("/downloads/android/latest.json", { cache: "no-store" })
      .then((r) => (r.ok ? (r.json() as Promise<AndroidRelease>) : null))
      .then((j) => (j && typeof j.url === "string" && typeof j.version === "string" ? j : null))
      .catch(() => null);
  }
  return cached;
}
