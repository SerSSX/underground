"use client";

import { useEffect, useState } from "react";
import { getFeatures, type Features } from "./api";

/** The API's runtime switches (TASK-304). Everything reads as off until the answer arrives (`ready`), so a page
 *  never offers code sign-in or "Forgot password?" on a deployment without mail. */
export function useFeatures(): Features & { ready: boolean } {
  const [f, setF] = useState<Features & { ready: boolean }>({ email: false, ready: false });
  useEffect(() => {
    let live = true;
    void getFeatures().then((v) => live && setF({ ...v, ready: true }));
    return () => {
      live = false;
    };
  }, []);
  return f;
}
