"use client";

import { useEffect, useState } from "react";
import { CircleCheck } from "./Icon";

/** Short confirmation at the bottom of the screen; disappears by itself. */
export function useToast(): [string | null, (msg: string) => void] {
  const [msg, setMsg] = useState<string | null>(null);
  useEffect(() => {
    if (!msg) return;
    const t = setTimeout(() => setMsg(null), 2600);
    return () => clearTimeout(t);
  }, [msg]);
  return [msg, setMsg];
}

export function Toast({ msg }: { msg: string | null }) {
  if (!msg) return null;
  return <div className="toast" role="status"><CircleCheck className="ico sm" />{msg}</div>;
}
