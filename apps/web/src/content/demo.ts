// Example data for the app window on the homepage: a made-up sleep-sounds
// app, so the demo shows real AppDoctor screens without anyone's real data.

export const DEMO_APP = {
  name: "Moonwake: Sleep Sounds",
  developer: "Moonwake Labs",
  countries: ["us", "gb"] as const,
}

export type Country = (typeof DEMO_APP.countries)[number]

export type DemoKeyword = {
  term: string
  popularity: number
  difficulty: number
  /** Rank by country now, and the rank before; null when not in the results. */
  rank: Record<Country, { now: number | null; before: number | null }>
  /** Colours standing in for the top apps' icons. */
  top: [string, string, string]
}

const r = (now: number | null, before: number | null) => ({ now, before })

export const DEMO_KEYWORDS: DemoKeyword[] = [
  { term: "sleep sounds", popularity: 71, difficulty: 82, rank: { us: r(34, 39), gb: r(28, 28) }, top: ["#4f7cff", "#8b5cf6", "#0ea5e9"] },
  { term: "white noise", popularity: 68, difficulty: 79, rank: { us: r(null, null), gb: r(61, 70) }, top: ["#e5e7eb", "#6366f1", "#14b8a6"] },
  { term: "rain sounds", popularity: 55, difficulty: 61, rank: { us: r(19, 16), gb: r(12, 15) }, top: ["#0284c7", "#64748b", "#22c55e"] },
  { term: "sleep timer", popularity: 44, difficulty: 52, rank: { us: r(8, 10), gb: r(5, 6) }, top: ["#f59e0b", "#4f46e5", "#ef4444"] },
  { term: "baby sleep", popularity: 48, difficulty: 70, rank: { us: r(41, 41), gb: r(null, 88) }, top: ["#ec4899", "#f97316", "#a855f7"] },
  { term: "brown noise", popularity: 39, difficulty: 44, rank: { us: r(6, 9), gb: r(4, 4) }, top: ["#92400e", "#1f2937", "#78716c"] },
  { term: "ocean waves", popularity: 33, difficulty: 37, rank: { us: r(3, 3), gb: r(2, 3) }, top: ["#0891b2", "#2563eb", "#0d9488"] },
  { term: "calm night", popularity: 21, difficulty: 30, rank: { us: r(2, 4), gb: r(1, 1) }, top: ["#312e81", "#7c3aed", "#1e40af"] },
  { term: "sleep stories", popularity: 52, difficulty: 76, rank: { us: r(null, null), gb: r(null, null) }, top: ["#2563eb", "#16a34a", "#9333ea"] },
  { term: "nap timer", popularity: 18, difficulty: 22, rank: { us: r(5, 12), gb: r(9, 9) }, top: ["#ea580c", "#facc15", "#0284c7"] },
  { term: "fan noise", popularity: 27, difficulty: 35, rank: { us: r(14, 11), gb: r(22, 30) }, top: ["#475569", "#0f766e", "#334155"] },
  { term: "lullaby", popularity: 36, difficulty: 58, rank: { us: r(52, 60), gb: r(47, 47) }, top: ["#db2777", "#f472b6", "#8b5cf6"] },
]

/** The listing as it starts in the Localization view, for the reader to edit. */
export const DEMO_LISTING = {
  name: "Moonwake: Sleep Sounds",
  subtitle: "Rain, ocean & brown noise",
  keywords: "sleep,timer,white,noise,baby,lullaby,nap,fan,calm,night,relax,sounds",
}

/** Thirty days of US ranks for the Rankings chart, oldest first. */
export const DEMO_HISTORY: { term: string; color: string; ranks: number[] }[] = [
  { term: "sleep timer", color: "#7c7cff", ranks: [22, 21, 21, 19, 18, 18, 17, 16, 16, 15, 14, 14, 13, 13, 12, 12, 12, 11, 11, 11, 10, 10, 10, 10, 9, 9, 10, 9, 8, 8] },
  { term: "rain sounds", color: "#22c39a", ranks: [30, 29, 27, 27, 25, 24, 24, 22, 21, 21, 20, 19, 18, 18, 17, 17, 16, 16, 15, 15, 15, 16, 16, 16, 17, 17, 18, 18, 19, 19] },
  { term: "sleep sounds", color: "#f2b84b", ranks: [58, 57, 55, 54, 54, 52, 50, 49, 48, 47, 47, 45, 44, 44, 43, 42, 42, 41, 40, 40, 39, 39, 38, 38, 37, 36, 36, 35, 35, 34] },
]

export const DEMO_AUDIT = {
  score: 68,
  grade: "Fair",
  areas: [
    { label: "Listing text", score: 70 },
    { label: "Keywords", score: 74 },
    { label: "Screenshots and video", score: 80 },
    { label: "Ratings and updates", score: 60 },
    { label: "Countries and languages", score: 55 },
  ],
  findings: [
    { level: "warning", title: "Keyword field repeats words", detail: "“sleep”, “noise” and “sounds” are already in the name or subtitle. Apple counts each word once, so swap them for new ones." },
    { level: "warning", title: "2 tracked keywords aren't in your listing", detail: "Search can't match “sleep stories” or “ocean waves” because not all their words are in the name, subtitle or keyword field." },
    { level: "warning", title: "Countries you track have no listing in their language", detail: "Add English (U.K.): it's its own name, subtitle and keyword field for the UK App Store." },
    { level: "tip", title: "No app preview video", detail: "A 15–30 second preview autoplays in search. Make one in Screenshots, or ask the AI." },
  ],
}

/** The finished AI conversation in the chat panel. */
export const DEMO_CHAT = {
  asked: "Improve my listing for the keywords I track",
  steps: ["Checked your keywords and ranks", "Read your listing", "Audited your App Store page", "Suggested changes for English (U.S.)"],
  reply:
    "Your keyword field spends 18 characters on “sleep”, “noise” and “sounds”, which the name and subtitle already have. I suggested “stories” and “waves” in their place, so “sleep stories” and “ocean waves” become searchable. It's waiting for you in Localization.",
}
