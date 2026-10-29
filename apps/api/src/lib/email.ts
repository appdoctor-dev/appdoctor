import type { Bindings } from "../env";

type Email = { to: string; subject: string; text: string };

/** Sends through Resend when a key is set; otherwise logs, so local sign-in works offline. */
export async function sendEmail(env: Bindings, email: Email) {
  if (!env.RESEND_API_KEY) {
    console.log(`[email] to=${email.to} subject="${email.subject}"\n${email.text}`);
    return;
  }

  const res = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ from: env.EMAIL_FROM, ...email }),
  });
  if (!res.ok) {
    throw new Error(`Email send failed: ${res.status} ${await res.text()}`);
  }
}
