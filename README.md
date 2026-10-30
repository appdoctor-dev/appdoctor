# AppDoctor

An open-source, local-first ASO tool for macOS and Windows: App Store keyword research with Apple's own popularity score, rank tracking in every country, listing edits pushed to App Store Connect, and AI agents that can localize and submit for you. See [docs/product-spec.md](docs/product-spec.md).

Website and downloads: [appdoctor.so](https://appdoctor.so)

## Repository

| Path | What it is |
| --- | --- |
| `apps/desktop`, `crates/*` | The native app (Rust + GPUI) for macOS and Windows. See [apps/desktop/README.md](apps/desktop/README.md). |
| `apps/api` | Hono on Cloudflare Workers. Better Auth (magic link, Google, desktop device sign-in) with the Stripe plugin for subscriptions; D1 + R2. Admin routes are served only through a private service binding. |
| `apps/web` | Marketing site (TanStack Start): landing, pricing, download, account, docs, blog, and the agent layer (`llms.txt`, Markdown pages, sitemap, robots, RSS). |
| `apps/admin` | Admin (TanStack Start + shadcn): revenue, subscribers, users, pricing, releases, Stripe alerts. Protected by Cloudflare Access (Zero Trust). |
| `packages/db` | Drizzle schema and D1 migrations, shared by the API. |

## Build the desktop app

Requires [rustup](https://rustup.rs); it installs the Rust version pinned in `rust-toolchain.toml` on first use.

```bash
cargo run -p appdoctor-desktop --features no-license-check
```

`no-license-check` turns off the subscription check, so every feature works without an AppDoctor account. [apps/desktop/README.md](apps/desktop/README.md) covers the rest: tests, the macOS bundle, data folders and the architecture.

## Releases and automatic updates

The [Release workflow](.github/workflows/release.yml) builds one universal Mac app (Apple silicon and Intel) as `AppDoctor-macOS.dmg` and a Windows `AppDoctor-Windows-x64.zip`, and attaches both to a GitHub release. The website's download buttons always point at the newest release.

1. Once, before the first release, make the key that signs updates, then commit `apps/desktop/update-signing-key.pub`:

   ```bash
   apps/desktop/scripts/update-signing-key.sh
   ```

2. For each release, raise `version` in `Cargo.toml`, commit, then tag and push:

   ```bash
   git tag v0.2.0 && git push origin v0.2.0
   ```

Running the workflow by hand (Actions → Release → Run workflow) makes a draft release to try first.

Mac apps from a release look for a new one at launch and every six hours. They download it, check its signature against the key built into the app, install it over themselves and show **Restart to Update**. Windows apps show the new version with a Download button for now. To sign and notarize the Mac app, add the Apple secrets listed at the top of the workflow.

## Run the website, API and admin locally

Requires Node 24+.

```bash
npm install
```

Create the API's local secrets:

```bash
cp apps/api/.dev.vars.example apps/api/.dev.vars
```

Fill in `apps/api/.dev.vars`:

- `BETTER_AUTH_SECRET`: `openssl rand -base64 32`
- `STRIPE_SECRET_KEY`: a restricted key (`rk_test_…`) from a [Stripe sandbox](https://docs.stripe.com/sandboxes). It needs write access to Products, Prices, Customers, Checkout Sessions, Customer Portal and Subscriptions, and read access to Charges, Invoices and Invoice Payments.
- `LICENSE_SIGNING_KEY`: run `npm run keys:generate`. Keep the printed public key for the desktop app.

The admin uses Cloudflare Access instead of its own sign-in. Access can't sit in front of localhost, so locally it takes your identity from `apps/admin/.dev.vars`:

```bash
cp apps/admin/.dev.vars.example apps/admin/.dev.vars
```

Create the local database, then forward Stripe webhooks (needs the [Stripe CLI](https://docs.stripe.com/stripe-cli)) and copy the `whsec_…` it prints into `STRIPE_WEBHOOK_SECRET`:

```bash
npm run db:migrate:local
```

```bash
npm run stripe:listen
```

Start everything:

```bash
npm run dev
```

- Website: http://localhost:3000
- Admin: http://localhost:3001
- API: http://localhost:8787

Without `RESEND_API_KEY`, sign-in links are printed in the API's console instead of emailed. Open the admin, go to **Pricing** and create your first plan; it appears on the website's pricing page straight away.

Locally the admin dev server also runs the API inside it (so its private `API` binding works) and shares the API's local database.

## Deploy (Cloudflare)

1. The configs use `appdoctor.so`. To use another domain, change it in the three `wrangler.jsonc` files and `apps/web/.env.production`.
2. Create the database and bucket, and put the D1 `database_id` in `apps/api/wrangler.jsonc`:

   ```bash
   npx wrangler d1 create appdoctor
   ```

   ```bash
   npx wrangler r2 bucket create appdoctor-releases
   ```

3. Set the API's secrets (each one listed in `apps/api/.dev.vars.example`), from `apps/api`:

   ```bash
   npx wrangler secret put STRIPE_SECRET_KEY --env production
   ```

4. Apply migrations, then deploy all three workers:

   ```bash
   npm run db:migrate:remote
   ```

   ```bash
   npm run deploy
   ```

5. Cloudflare Access (Zero Trust) protects the admin. The Access application "AppDoctor admin" for `admin.appdoctor.so` uses the reusable "Owner" policy. Its team domain and AUD tag are in `apps/admin/wrangler.jsonc`, and the admin Worker verifies the Access token on every request (it refuses requests without one). The admin has no workers.dev or preview URL, so Access can't be bypassed. The API's admin routes aren't on the internet at all: the admin reaches them through a private service binding (`AdminEntrypoint`).

6. In Stripe (live mode):
   - Add a webhook endpoint at `https://api.appdoctor.so/api/auth/stripe/webhook`. Subscribe it to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `charge.refunded`, `charge.dispute.created` and `radar.early_fraud_warning.created`.
   - Configure the [Customer Portal](https://dashboard.stripe.com/settings/billing/portal) so subscribers can cancel and update their card.

### Sales tax and VAT

Selling software subscriptions to people in the US and EU usually means collecting tax. `STRIPE_AUTOMATIC_TAX=true` turns on Stripe Tax in Checkout, but Stripe collects nothing until you add your tax registrations (Dashboard → Tax → Registrations). Talk to a tax advisor about where you need to register, especially for EU VAT.

## For AI agents

The website serves `/llms.txt`, `/llms-full.txt` and a Markdown version of every page (append `.md`, or send `Accept: text/markdown`).

## License

[AGPL-3.0](LICENSE). You can read, build and change AppDoctor for free. If you run a changed version as a service for other people, you have to publish your changes under the same licence.
