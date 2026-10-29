import { createFileRoute } from "@tanstack/react-router"
import { PageIntro } from "#/components/chrome"
import { PRIVACY_POINTS, SITE } from "#/content/site"

export const Route = createFileRoute("/privacy")({
  head: () => ({ meta: [{ title: `Privacy: ${SITE.name}` }, { name: "description", content: `What ${SITE.name} stores, where, and who it talks to.` }] }),
  component: Privacy,
})

// Plain-language summary of how the product handles data. Have it reviewed
// before launch and add a full policy if your jurisdiction requires one.
function Privacy() {
  return (
    <>
      <PageIntro title="Privacy">{SITE.name} is built so that what you research stays yours.</PageIntro>
      <div className="prose-doc mx-auto max-w-6xl px-4 pb-20 sm:px-6">
        <h2>In short</h2>
        <ul>
          {PRIVACY_POINTS.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
        <h2>What we store</h2>
        <p>
          Your email address, your sign-in sessions, your subscription status from Stripe, and the computers you've signed
          in on (a random ID the app creates, the name you gave the computer, its platform and the app version). Payments
          are handled by Stripe; we never see your card.
        </p>
        <h2>What stays on your computer</h2>
        <p>
          Your apps, keywords, rankings and their history, your App Store Connect and Apple Ads keys, and anything you ask an
          AI agent to do with them. If you use an AI provider, the text it needs goes to the provider you chose.
        </p>
        <h2>Contact</h2>
        <p>
          Questions or deletion requests: <a href={`mailto:${SITE.email}`}>{SITE.email}</a>.
        </p>
      </div>
    </>
  )
}
