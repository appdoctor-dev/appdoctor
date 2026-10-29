// One place for the words on the site. Pages render these, and the agent layer
// (llms.txt, the Markdown versions of pages, JSON-LD) reads the same objects, so
// what people see and what agents read can't drift apart.

export const SITE = {
  name: "AppDoctor",
  // Search terms this site is written for: "aso tool", "aso keyword research tools".
  title: "AppDoctor: the ASO tool for keyword research and rank tracking",
  description:
    "AppDoctor is an ASO tool for Mac and Windows. Research App Store keywords with Apple's own search popularity, track your rank in every country, and update your listing in App Store Connect. Your data stays on your computer.",
  // TODO: set to the public repository once it exists.
  sourceUrl: "https://github.com/appdoctor-app/appdoctor",
  email: "hello@appdoctor.so",
}

export const NAV = [
  { to: "/pricing", label: "Pricing" },
  { to: "/docs", label: "Docs" },
  { to: "/blog", label: "Blog" },
  { to: "/download", label: "Download" },
] as const

export type Feature = { id: string; title: string; body: string; soon?: boolean }

export const FEATURES: Feature[] = [
  {
    id: "keywords",
    title: "Keyword research with Apple's popularity score",
    body: "Connect your Apple Ads account and every keyword shows Apple's own 0–100 search popularity, next to how hard it is to reach the top 10 and ideas for related searches.",
  },
  {
    id: "ranks",
    title: "Rank tracking in every App Store country",
    body: "Check where your app ranks for each keyword, in each country, every day. History is kept, so you can see what a listing change actually did.",
  },
  {
    id: "listing",
    title: "Edit your listing and send it to App Store Connect",
    body: "Rewrite your name, subtitle and keyword field with live character counts, then push the change to a draft version in App Store Connect.",
  },
  {
    id: "agents",
    title: "Let your AI agent localize your app",
    body: "Connect Claude, Codex or any MCP client and ask it to localize your listing into the languages you choose. It asks before changing anything.",
    soon: true,
  },
  {
    id: "design",
    title: "Screenshots and preview videos",
    body: "Put your screens in device frames, write the captions, and animate App Store preview videos, without leaving the app.",
    soon: true,
  },
  {
    id: "submit",
    title: "Submit for review",
    body: "Pick a build, attach it to the version and send it to App Review, yourself or through your agent with a confirmation step.",
    soon: true,
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
    q: "What is an ASO tool?",
    a: "App Store Optimization (ASO) is choosing the words in your app's name, subtitle and keyword field so people find it when they search the App Store. An ASO tool shows which searches are popular, how hard they are to rank for, and where your app ranks today.",
  },
  {
    q: "Where does the keyword popularity come from?",
    a: "From Apple. Apple Ads publishes a 0–100 popularity score for search phrases, and AppDoctor reads it with your own Apple Ads account, from your computer.",
  },
  {
    q: "Do I need to be technical?",
    a: "No. You install the app, sign in with your email, and add your app by pasting its App Store link. Connecting App Store Connect and Apple Ads is a guided step with screenshots.",
  },
  {
    q: "It's open source. Why pay?",
    a: "You can build AppDoctor from the source code for free. The subscription gets you signed, ready-to-install apps for Mac and Windows, automatic updates and support.",
  },
  {
    q: "Can I cancel?",
    a: "Yes, any time from your account page. You keep access until the end of the period you paid for.",
  },
]
