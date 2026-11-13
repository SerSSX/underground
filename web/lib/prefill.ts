// Hand an email address from one page to the next without putting it in the URL (TASK-304): sign-up with an
// address that already has an account sends the person to /login with it filled in. Session storage only, read
// once; a URL would keep the address in history and in server logs.

const KEY = "ug_prefill_email";

export function setPrefillEmail(email: string): void {
  try {
    window.sessionStorage.setItem(KEY, email);
  } catch {
    /* storage off: the login form just starts empty */
  }
}

export function takePrefillEmail(): string | null {
  try {
    const v = window.sessionStorage.getItem(KEY);
    window.sessionStorage.removeItem(KEY);
    return v && v.includes("@") && v.length <= 320 ? v : null;
  } catch {
    return null;
  }
}
