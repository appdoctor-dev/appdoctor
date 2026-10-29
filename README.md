# AppDoctor

An open-source, local-first ASO tool for macOS and Windows: App Store keyword research with Apple's own popularity score, rank tracking in every country, listing edits pushed to App Store Connect, and AI agents that can localize and submit for you. See [docs/product-spec.md](docs/product-spec.md).

## Repository

| Path | What it is |
| --- | --- |
| `apps/api` | Hono on Cloudflare Workers. Better Auth (magic link, Google, desktop device sign-in, admin roles) with the Stripe plugin for subscriptions; D1 + R2. |
| `apps/web` | Marketing site (TanStack Start): landing, pricing, download, account, docs, blog, and the agent layer (`llms.txt`, Markdown pages, sitemap, robots, RSS). |
| `apps/admin` | Admin (TanStack Start + shadcn): revenue, subscribers, users, pricing, releases, Stripe alerts. |
| `packages/db` | Drizzle schema and D1 migrations, shared by the API. |
| `apps/desktop`, `crates/*` | The native app (Rust + GPUI), on its own branch while it's being built. |

## Run it locally

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
- `ADMIN_EMAILS`: your email. Addresses listed here become admins the first time they sign in.

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

Without `RESEND_API_KEY`, sign-in links are printed in the API's console instead of emailed. Sign in to the admin, open **Pricing** and create your first plan; it appears on the website's pricing page straight away.

## Deploy (Cloudflare)

1. Replace `appdoctor.app` in the three `wrangler.jsonc` files and the two `.env.production` files with your domain.
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

5. In Stripe (live mode):
   - Add a webhook endpoint at `https://api.<your-domain>/api/auth/stripe/webhook`. Subscribe it to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `charge.refunded`, `charge.dispute.created` and `radar.early_fraud_warning.created`.
   - Configure the [Customer Portal](https://dashboard.stripe.com/settings/billing/portal) so subscribers can cancel and update their card.

### Sales tax and VAT

Selling software subscriptions to people in the US and EU usually means collecting tax. `STRIPE_AUTOMATIC_TAX=true` turns on Stripe Tax in Checkout, but Stripe collects nothing until you add your tax registrations (Dashboard → Tax → Registrations). Talk to a tax advisor about where you need to register, especially for EU VAT.

## For AI agents

The website serves `/llms.txt`, `/llms-full.txt` and a Markdown version of every page (append `.md`, or send `Accept: text/markdown`).
