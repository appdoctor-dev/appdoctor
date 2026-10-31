import type { Bindings } from "../env";

type Email = { to: string; subject: string; text: string };

/**
 * Sends with Cloudflare Email Sending through the EMAIL binding, which only
 * production has; locally it logs, so sign-in works offline.
 */
export async function sendEmail(env: Bindings, email: Email) {
  if (!env.EMAIL) {
    console.log(`[email] to=${email.to} subject="${email.subject}"\n${email.text}`);
    return;
  }
  await env.EMAIL.send({ from: sender(env.EMAIL_FROM), ...email });
}

/** "AppDoctor <hello@appdoctor.so>" as the binding wants it: { name, email }. */
function sender(from: string) {
  const named = from.match(/^(.+?)\s*<(.+)>$/);
  return named ? { name: named[1]!, email: named[2]! } : from;
}
