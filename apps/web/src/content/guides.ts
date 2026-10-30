// The two long-form guides, written for the searches "aso keyword research"
// and "aso keyword research tools". The pages, their Markdown versions,
// llms.txt and the Article and FAQPage JSON-LD all read these objects.
//
// Keep claims to what Apple documents or AppDoctor does; no competitor
// names, prices or statistics we can't source.

export type Block =
  | { kind: "p"; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "steps"; items: { title: string; text: string }[] }
  | { kind: "table"; caption: string; head: string[]; rows: string[][] }

export type Section = { id: string; heading: string; blocks: Block[] }

export type Guide = {
  path: "/aso-keyword-research" | "/aso-keyword-research-tools"
  /** The <title>, with the search phrase first. */
  title: string
  h1: string
  description: string
  /** ISO date of the last real edit. */
  updated: string
  intro: string[]
  sections: Section[]
  faq: { q: string; a: string }[]
}

export const KEYWORD_RESEARCH: Guide = {
  path: "/aso-keyword-research",
  title: "ASO keyword research: how to find App Store keywords that bring installs",
  h1: "ASO keyword research, step by step",
  description:
    "A practical guide to App Store keyword research: what popularity, difficulty and relevance mean, which parts of your listing Apple searches, and how to tell whether your keywords work.",
  updated: "2026-10-30",
  intro: [
    "ASO keyword research is finding the words people type into App Store search, choosing the ones your app can win and that describe what it really does, and putting them in the parts of your listing Apple searches.",
    "Most App Store downloads start with a search, so the words in your name, subtitle and keyword field decide which searches you can appear in at all. This guide walks through the whole loop, from a blank page to a listing you can measure.",
  ],
  sections: [
    {
      id: "numbers",
      heading: "The four things to know about every keyword",
      blocks: [
        {
          kind: "table",
          caption: "What to check for each keyword",
          head: ["", "What it tells you", "Where it comes from"],
          rows: [
            ["Popularity", "How often people search for it.", "Apple Ads gives a popularity score for search terms, from Apple's own search data."],
            ["Difficulty", "How hard it is to reach the top 10.", "The strength of the apps already ranking: their ratings, reviews and how well their listings match."],
            ["Relevance", "Whether someone searching it wants your app.", "You. Only you know if the app does what the searcher is looking for."],
            ["Your rank", "Where you appear today, in each country.", "A search of the App Store in that country."],
          ],
        },
        {
          kind: "p",
          text: "The keywords worth working on are relevant first, then popular enough to matter and easy enough to win. A popular term you can't reach brings nothing; a modest one where you can be third brings installs every day.",
        },
      ],
    },
    {
      id: "steps",
      heading: "How to do it",
      blocks: [
        {
          kind: "steps",
          items: [
            {
              title: "Write down what your app is for",
              text: "List the problems it solves and the words your users use for them: in reviews, support emails and how they describe the app to a friend. These are your seed terms.",
            },
            {
              title: "Widen the list",
              text: "Type each seed term into App Store search and note what it suggests. Look at the names and subtitles of the apps that rank for it. Ask Apple Ads for related terms. Aim for 30 to 60 candidates.",
            },
            {
              title: "Score every candidate",
              text: "Check popularity, difficulty and your current rank. Mark anything that doesn't fit the app and drop it. Longer phrases of two or three words are usually easier to rank for and convert better, because the searcher knows what they want.",
            },
            {
              title: "Place the best ones in your listing",
              text: "Your strongest keyword goes in the name, the next ones in the subtitle, and the rest in the keyword field. Apple combines words across these three fields, so a word only needs to appear once.",
            },
            {
              title: "Repeat for every language you sell in",
              text: "Each localization is its own name, subtitle and keyword field, and each storefront reads the listing in its own language. Translate the meaning, then research the local words; a direct translation is often not what people search.",
            },
            {
              title: "Track, then change one thing at a time",
              text: "Check your ranks in each country every week. Keyword changes go live with your next version, so change a few things, release, wait a week or two, and compare.",
            },
          ],
        },
      ],
    },
    {
      id: "fields",
      heading: "Where keywords count",
      blocks: [
        {
          kind: "table",
          caption: "Which listing fields App Store search reads on iPhone and iPad",
          head: ["Field", "Searched", "Limit"],
          rows: [
            ["App name", "Yes, and it weighs the most", "30 characters"],
            ["Subtitle", "Yes", "30 characters"],
            ["Keyword field", "Yes, and nobody sees it", "100 characters"],
            ["In-app purchase names", "Promoted in-app purchases can appear in search", "30 characters"],
            ["Promotional text", "No", "170 characters"],
            ["Description", "No", "4,000 characters"],
          ],
        },
        {
          kind: "list",
          items: [
            "Separate keywords with commas and no spaces: spaces spend characters search doesn't need.",
            "Don't repeat a word that's already in the name or subtitle.",
            "Words like “app” and “free” rarely win a search on their own.",
            "Promotional text and the description aren't searched, but they're what people read before they tap Get, so write them for people.",
          ],
        },
      ],
    },
    {
      id: "mistakes",
      heading: "Mistakes that cost rankings",
      blocks: [
        {
          kind: "list",
          items: [
            "Chasing only the most popular terms, where the top 10 belongs to apps with hundreds of thousands of ratings.",
            "Repeating words across the name, subtitle and keyword field, which wastes characters.",
            "Putting other apps' names or trademarks in your metadata, which App Review guideline 2.3.7 doesn't allow.",
            "Turning the app name into a list of keywords. Apple can reject it, and people don't trust it.",
            "Changing every field at once, so you can't tell what helped.",
            "Researching one country and copying the result to the rest.",
          ],
        },
      ],
    },
    {
      id: "appdoctor",
      heading: "Doing it with AppDoctor",
      blocks: [
        {
          kind: "p",
          text: "AppDoctor does each step on your Mac or PC. It reads Apple's popularity score through your own Apple Ads account, works out difficulty from the apps that rank today, and checks your rank in every country you choose, keeping the history. Its listing editor counts characters, flags repeated words and shows which tracked keywords your listing can't match yet, then saves the change to App Store Connect. An audit scores the whole product page, and your own AI (Claude Code, Codex, Gemini CLI or a model in Ollama) can suggest the edits for you to review.",
        },
      ],
    },
  ],
  faq: [
    {
      q: "How many keywords should I track?",
      a: "Between 15 and 30 per country is a good start: a few popular terms you're aiming for, and more specific ones you can win now. Track more once you know which kind works for your app.",
    },
    {
      q: "How long do keyword changes take to work?",
      a: "Changes to the name, subtitle and keyword field go live with your next approved version. Rankings usually settle within a few days to two weeks after that, so compare ranks a week or two after release.",
    },
    {
      q: "Does Apple search the app description?",
      a: "Not on the App Store for iPhone and iPad. Search reads the app name, subtitle and keyword field. The description is for people deciding whether to download.",
    },
    {
      q: "Where does keyword popularity come from?",
      a: "Apple Ads publishes a popularity score for search terms, based on Apple's own search data. AppDoctor reads it with your Apple Ads account, from your computer.",
    },
    {
      q: "Should I use the same keywords in every country?",
      a: "No. Each storefront reads the listing in its own language and people search differently in each market. Research each language you sell in, and add a localization for countries where you have none.",
    },
  ],
}

export const KEYWORD_TOOLS: Guide = {
  path: "/aso-keyword-research-tools",
  title: "ASO keyword research tools: what they do and how to choose one",
  h1: "ASO keyword research tools: what to look for",
  description:
    "What an ASO keyword research tool should give you, from Apple's own popularity data and difficulty to ranks by country and help with your listing, free ways to start, and how to choose.",
  updated: "2026-10-30",
  intro: [
    "An ASO keyword research tool tells you which App Store searches are worth your app's name, subtitle and keyword field, and whether the ones you picked are working.",
    "They differ most in where their numbers come from, which countries they cover, where your data is kept and who they're built for. Here's what to check before you pay for one.",
  ],
  sections: [
    {
      id: "jobs",
      heading: "What a keyword research tool does",
      blocks: [
        {
          kind: "list",
          items: [
            "Finds searches: suggestions from a seed word, related terms and what competing apps rank for.",
            "Sizes them: how often each one is searched.",
            "Judges your chances: how strong the apps in the top 10 are.",
            "Tracks results: your rank for each keyword, in each country, over time.",
            "Helps you write the listing: character counts, repeated words and which keywords your listing covers.",
          ],
        },
      ],
    },
    {
      id: "criteria",
      heading: "What to look for",
      blocks: [
        {
          kind: "table",
          caption: "Questions to ask of any ASO keyword research tool",
          head: ["Question", "Why it matters"],
          rows: [
            ["Where does popularity come from?", "Apple publishes its own popularity score through Apple Ads. A tool that uses it shows Apple's numbers; others model their own estimates."],
            ["How is difficulty worked out?", "A useful score is based on the apps that rank today, and you should be able to see those apps."],
            ["Which countries can it check?", "Every storefront ranks differently. You want ranks in each country you sell in, with history."],
            ["Does it help with the listing?", "Research only pays off once the words are in your name, subtitle and keyword field, so live counts, repeated-word checks and keyword coverage save time."],
            ["Does it handle languages?", "Each localization is its own set of keywords. A tool should show which countries you track have no listing in their language."],
            ["Where is your data kept?", "Some tools keep your keywords and App Store Connect access in their cloud; others keep them on your computer."],
            ["Can your AI use it?", "Tools with an MCP server let Claude, Codex or another agent read your keywords and draft changes."],
            ["What does it cost a small team?", "Many tools are priced for agencies. Check the price of the plan with the countries and keywords you need."],
          ],
        },
      ],
    },
    {
      id: "free",
      heading: "Free ways to start",
      blocks: [
        {
          kind: "list",
          items: [
            "App Store search suggestions: type a word into search on your iPhone and note what it offers.",
            "Apple Ads: its keyword recommendations and popularity score come from Apple's own search data.",
            "Competitors' listings: the names and subtitles of the apps that rank show the words that work.",
            "App Store Connect analytics: the App Store Search source shows how many views and downloads come from search.",
          ],
        },
        {
          kind: "p",
          text: "A spreadsheet and these sources are enough to start. A tool earns its place when you track enough keywords, in enough countries, that checking by hand stops being practical.",
        },
      ],
    },
    {
      id: "kinds",
      heading: "The kinds of tools",
      blocks: [
        {
          kind: "table",
          caption: "Common kinds of ASO keyword research tools",
          head: ["Kind", "Good for", "Keep in mind"],
          rows: [
            ["Web ASO suites", "Teams and agencies that want many apps, markets and app stores in one dashboard.", "Your data lives in their cloud, and pricing is often per seat or per app."],
            ["Spreadsheets with Apple Ads", "Trying ASO for the first time, with a handful of keywords.", "Ranks are checked by hand, one country at a time."],
            ["Desktop apps such as AppDoctor", "Developers who want Apple's numbers through their own accounts and their data on their own computer.", "They run on your Mac or PC rather than in a browser."],
          ],
        },
      ],
    },
    {
      id: "appdoctor",
      heading: "Where AppDoctor fits",
      blocks: [
        {
          kind: "p",
          text: "AppDoctor is an ASO keyword research tool for Mac and Windows, made for indie developers and small teams on the App Store. It shows Apple's popularity score through your Apple Ads account, difficulty from the apps that rank today, and your rank in every country you choose. It also edits your listing in App Store Connect, audits your product page, designs screenshots and preview videos, and works with your own AI. Your keywords and keys stay on your computer, and the code is open source.",
        },
        {
          kind: "p",
          text: "If you need Google Play data, or market research across thousands of apps you don't own, a web suite is the better fit: AppDoctor covers the App Store only.",
        },
      ],
    },
  ],
  faq: [
    {
      q: "What is the best ASO keyword research tool?",
      a: "The one whose numbers you trust and whose price fits. Check where its popularity comes from, whether it covers every country you sell in, and where it keeps your data. For a single developer, a tool that uses Apple's own popularity score and checks ranks in your countries covers most of the work.",
    },
    {
      q: "Can I do ASO keyword research for free?",
      a: "Yes. App Store search suggestions, Apple Ads popularity and your competitors' listings are free. A tool saves time once you track more than a handful of keywords or countries.",
    },
    {
      q: "Are keyword popularity numbers accurate?",
      a: "Apple's own score, from Apple Ads, comes from real App Store searches. Other tools estimate popularity with their own models, so their numbers can differ. Use any score to compare keywords with each other rather than as an exact search count.",
    },
    {
      q: "Does AppDoctor work for Google Play?",
      a: "No. AppDoctor covers the Apple App Store: iPhone, iPad, and the product page header and search art.",
    },
  ],
}

export const GUIDES = [KEYWORD_RESEARCH, KEYWORD_TOOLS]
