"use client";

import { useState } from "react";
import { categoryGradient } from "../../lib/ui";

/** Event image, or — when there is none or it fails to load — a gradient in the category's hue with the
 *  title, so a card is never an empty black box. */
export default function Poster({ src, title, category, eager = false }: { src: string | null | undefined; title: string; category: string | null | undefined; eager?: boolean }) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) {
    return (
      <div className="poster-fallback" style={{ background: categoryGradient(category) }} aria-hidden>
        <span>{title}</span>
      </div>
    );
  }
  return (
    <img
      src={src}
      alt=""
      loading={eager ? "eager" : "lazy"}
      decoding="async"
      onError={() => setFailed(true)}
      style={{ background: categoryGradient(category) }}
    />
  );
}
