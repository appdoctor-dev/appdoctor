# AppDoctor product spec

AppDoctor is an open-source, local-first ASO (App Store Optimization) tool for macOS and Windows. The goal: everything Astro and Wow Keywords do, done better, plus agent-driven localization, creative tools and build submission. The code is public; non-technical buyers pay a monthly subscription for signed installers, automatic updates and support.

Marketing search terms: **aso tool**, **aso keyword research tools**.

## Principles

- **Fast like Cap.** Native Rust UI (GPUI + gpui-component), no webview. Cap's switch from Tauri to GPUI cut editor memory by 88%.
- **Local-first and private.** Keywords, rankings, history and Apple credentials live on the user's computer (SQLite, plus the OS keychain for secrets). The app talks to Apple directly. Our server knows only the email address, the subscription and the signed-in devices.
- **Apple's numbers, not guesses.** Popularity comes from the Apple Ads Platform API (1.0, Aug 2026). `POST /v1/suggestions/phrases/query` with `queryType=SEARCH` returns Apple's 0–100 popularity for phrases we choose. The old Campaign Management API sunsets on 2027-01-26.
- **Agents are first-class.** A local MCP server lets the user's own agent (Claude, Codex, Cursor, Ollama…) do anything the UI can. Outward-facing actions (listing upload, submit for review) require an explicit confirmation.
- **Non-technical buyers.** Passwordless sign-in, device-code login for the app, and guided steps for connecting App Store Connect and Apple Ads.

## Feature list

### Keyword research (parity: Astro, Wow Keywords; better: official popularity)
- Apple popularity (0–100) per keyword, from the user's Apple Ads account
- Difficulty (top-10 strength: ratings, rating average, title/subtitle matches)
- Keyword suggestions: Apple Ads keyword/phrase suggestions, competitor-derived, long-tail, AI brainstorm from a niche description
- Category trend lists (Apple's weekly/monthly top search terms by genre and country)
- Multi-country (every App Store storefront), local-language phrasings, English translation on hover
- Bulk add and check; import from Astro

### Rank tracking
- Daily ranks per keyword per country, history charts, on-demand checks
- Top-10 status, change since last check

### Competitors
- Track any app; which keywords they rank for; where they beat you; top-10 presence by market

### Listing optimization
- Edit name, subtitle and keyword field with character counts and reach/difficulty estimates
- Compare current vs proposed listings, ASO audit
- Push to a draft App Store version via App Store Connect (never straight to live)

### Localization by agent
- "Localize my app into these languages": the agent drafts each locale's name, subtitle, keywords and description using per-country research, then saves them to App Store Connect drafts after confirmation
- Version history for each language: what was pulled and saved, typed edits, and named versions, with what differs from the fields; restoring fills the fields, and only Review and Save changes App Store Connect

### Screenshot designer
- Device frames (current iPhone/iPad/Mac sizes), text and caption editor, backgrounds, layout presets, all App Store sizes
- AI "art director" that builds a screenshot from a prompt (concept from the user's appshots project; new code)
- Version history for each design: versions save as you work and before an AI's edits, you can name one to keep it, click any to see it on the canvas, and restore it (undoable)

### App preview video designer
- Animate screens in device frames with captions and motion; export App Store preview specs (resolution, length, H.264)

### Build submission
- List builds, attach one to a version, create the review submission and submit, from the UI or the agent, always behind a confirmation summary
- Optional: upload the .ipa/.pkg (App Store Connect build-upload API if available, else `altool`/Transporter on macOS)

### Ratings and reviews
- Scan storefronts for new reviews; ratings analysis

### AI
- Bring your own provider (Claude Code, Codex, Gemini, Cursor, OpenCode, Grok) or a local Ollama model; no AI credits sold
- MCP server exposing keyword data, ranks, listing edits, localization and submission

## Business and web

- `apps/api`: Hono on Cloudflare Workers. Better Auth (magic link, Google, device authorization for the desktop app, bearer tokens, admin roles) with the Stripe plugin for subscriptions. D1 (SQLite) via Drizzle. Signed Ed25519 entitlements let the app run offline for up to 7 days.
- `apps/web`: TanStack Start marketing site, which counts visits without cookies for the admin's analytics. Landing, pricing (Stripe Checkout), download, account (billing portal, devices), device approval, docs and blog in Markdown. Agent-ready: `llms.txt`, `llms-full.txt`, Markdown for every page (`.md` or `Accept: text/markdown`), sitemap, `robots.txt` with Content-Signal, RSS, JSON-LD, Link headers.
- `apps/admin`: TanStack Start + shadcn, behind Cloudflare Access (Zero Trust); the API's admin routes are only reachable through a private service binding. Dashboard (est. MRR, actives, trials, past due, churn), subscribers, users, devices, pricing editor (creates Stripe products/prices, archives old prices), site analytics (country, source, landing page, AI agents, Google Search Console queries), GitHub releases with download counts, and Stripe risk alerts (refunds, disputes, early fraud warnings).
- `apps/desktop` + `crates/*`: the GPUI app (built in a parallel session on its own branch).

## Open decisions

- Open-source license (AGPL-3.0 like Cap, or another)
- Exactly what the paid build unlocks versus a source build
- Google OAuth app
- Stripe Tax registrations before charging EU/US customers
