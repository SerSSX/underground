"use client";

import { useState } from "react";
import { categoryGradient } from "../../lib/ui";
import { smallVariant } from "../../lib/images.mjs";

/** Event image, or — when there is none or it fails to load — a gradient in the category's hue with the
 *  title, so a card is never an empty black box.
 *  `variant` (TASK-304): "card" lets the browser pick the 480 px copy or the 1600 px picture by `sizes`;
 *  "thumb" always takes the 480 px copy; "large" (event page) the full picture. Pictures without a copy (older
 *  uploads, external links) load as they are. If the copy fails, the full picture is tried before the gradient. */
export default function Poster({
  src,
  title,
  category,
  eager = false,
  variant = "large",
  sizes = "(max-width: 560px) calc(100vw - 32px), (max-width: 960px) 45vw, 360px",
}: {
  src: string | null | undefined;
  title: string;
  category: string | null | undefined;
  eager?: boolean;
  variant?: "card" | "thumb" | "large";
  sizes?: string;
}) {
  const [failed, setFailed] = useState(false);
  const [noCopy, setNoCopy] = useState(false);
  if (!src || failed) {
    return (
      <div className="poster-fallback" style={{ background: categoryGradient(category) }} aria-hidden>
        <span>{title}</span>
      </div>
    );
  }
  const small = variant !== "large" && !noCopy ? smallVariant(src) : null;
  return (
    <img
      src={small ?? src}
      srcSet={small && variant === "card" ? `${small} 480w, ${src} 1600w` : undefined}
      sizes={small && variant === "card" ? sizes : undefined}
      alt=""
      loading={eager ? "eager" : "lazy"}
      decoding="async"
      onError={() => (small ? setNoCopy(true) : setFailed(true))}
      style={{ background: categoryGradient(category) }}
    />
  );
}
