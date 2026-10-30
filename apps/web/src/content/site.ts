// One place for the words on the site. Pages render these, and the agent layer
// (llms.txt, the Markdown versions of pages, JSON-LD) reads the same objects, so
// what people see and what agents read can't drift apart.

export const SITE = {
  name: "AppDoctor",
  url: "https://appdoctor.so",
  // Search terms this site is written for: "aso keyword research tool",
  // "aso keyword research", "aso keyword research tools", "aso tool".
  title: "AppDoctor: ASO keyword research tool for the App Store",
  description:
    "AppDoctor is an ASO keyword research tool for Mac and Windows. Find App Store keywords with Apple's own popularity score, track your rank in every country, audit your listing and let your own AI improve it. Your data stays on your computer.",
  // TODO: set to the public repository once it exists.
  sourceUrl: "https://github.com/appdoctor-app/appdoctor",
  email: "hello@appdoctor.so",
}

export const NAV = [
  { to: "/aso-keyword-research", label: "Keyword research" },
  { to: "/pricing", label: "Pricing" },
  { to: "/docs", label: "Docs" },
  { to: "/blog", label: "Blog" },
] as const

export type Feature = { id: string; title: string; body: string; soon?: boolean }

export const FEATURES: Feature[] = [
  {
    id: "keywords",
    title: "Keyword research with Apple's popularity score",
    body: "Every keyword shows Apple's own search popularity, read through your Apple Ads account, next to how hard the top 10 is to break into and the apps already there.",
  },
  {
    id: "ranks",
    title: "Rank tracking in every App Store country",
    body: "Track the same app in the US, the UK or any storefront. AppDoctor checks where you rank for each keyword and keeps the history, so you can see what a change did.",
  },
  {
    id: "listing",
    title: "A listing editor that knows your keywords",
    body: "Edit the name, subtitle and keyword field in every language with live counts. It flags repeated words and the tracked keywords your listing can't match, then saves to App Store Connect after you confirm.",
  },
  {
    id: "audit",
    title: "An ASO audit for the whole product page",
    body: "A score out of 100 across listing text, keywords, screenshots and video, ratings and updates, and countries and languages, with what to fix first.",
  },
  {
    id: "ai",
    title: "Your own AI, inside the app",
    body: "Claude Code, Codex, Gemini CLI or a model in Ollama reads your keywords, ranks and listing and suggests changes. Nothing reaches the App Store until you review and save it.",
  },
  {
    id: "design",
    title: "Screenshots and app preview videos",
    body: "Design screenshots in real device frames for every iPhone and iPad, the new product page header and search art, and preview videos that export at Apple's sizes.",
  },
  {
    id: "submit",
    title: "Submit for App Review",
    body: "Pick a build, attach it to the version and send it to App Review from the app, with a summary to confirm first.",
  },
]

export const PRIVACY_POINTS = [
  "Your keywords, rankings and App Store Connect key are stored on your computer, not on our servers.",
  "Searches and rank checks go from your computer to Apple directly.",
  "Our server only knows your email address, your subscription and which devices are signed in.",
  "The code is open source, so you can check all of this yourself.",
]

export const FAQ = [
  {
    q: "What is an ASO keyword research tool?",
    a: "App Store Optimization (ASO) is choosing the words in your app's name, subtitle and keyword field so people find it when they search the App Store. An ASO keyword research tool shows which searches are popular, how hard they are to rank for, and where your app ranks today in each country.",
  },
  {
    q: "Where does the keyword popularity come from?",
    a: "From Apple. Apple Ads publishes a popularity score for search terms, and AppDoctor reads it with your own Apple Ads account, from your computer.",
  },
  {
    q: "Can I track my app in more than one country?",
    a: "Yes. Add the app from any App Store country, the UK or Japan say, and AppDoctor tracks its keywords and ranks there too, with a listing for each language.",
  },
  {
    q: "Which AI does it work with?",
    a: "The one you already have: Claude Code, Codex, Gemini CLI or a model in Ollama, inside the app. Claude Desktop, Cursor and other MCP apps can use AppDoctor's tools too. There's no second AI subscription.",
  },
  {
    q: "Do I need to be technical?",
    a: "No. You install the app, sign in with your email and add your app by pasting its App Store link. Connecting App Store Connect and Apple Ads is a guided step.",
  },
  {
    q: "It's open source. Why pay?",
    a: "You can build AppDoctor from the source code for free. The subscription gets you signed, ready-to-install apps for Mac and Windows, automatic updates and support.",
  },
]
