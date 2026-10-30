// Tells IndexNow (Bing, and through it Copilot and DuckDuckGo, plus Yandex,
// Naver, Seznam and others) that the site's pages changed, so they're
// recrawled within hours instead of weeks. Run after a deploy:
//   SITE_URL=https://appdoctor.so npm run indexnow
// The key is INDEXNOW_KEY in wrangler.jsonc; the site serves it at /{key}.txt.

import { readFileSync } from "node:fs"

const site = (process.env.SITE_URL ?? "https://appdoctor.so").replace(/\/$/, "")
const config = readFileSync(new URL("../wrangler.jsonc", import.meta.url), "utf8")
const key = /"INDEXNOW_KEY":\s*"([0-9a-f]+)"/.exec(config)?.[1]
if (!key) throw new Error("No INDEXNOW_KEY in wrangler.jsonc")

const sitemap = await (await fetch(`${site}/sitemap.xml`)).text()
const urlList = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1])
if (urlList.length === 0) throw new Error(`No URLs in ${site}/sitemap.xml`)

const response = await fetch("https://api.indexnow.org/indexnow", {
  method: "POST",
  headers: { "content-type": "application/json; charset=utf-8" },
  body: JSON.stringify({ host: new URL(site).host, key, keyLocation: `${site}/${key}.txt`, urlList }),
})
console.log(`IndexNow: ${response.status} for ${urlList.length} URLs`)
if (!response.ok && response.status !== 202) process.exit(1)
