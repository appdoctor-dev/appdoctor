import type { Guide } from "#/content/guides"
import { SITE } from "#/content/site"

// Schema.org for search engines and answer engines: who makes AppDoctor,
// what it is, and what each page is. Kept to types search engines use.

const url = (path: string) => `${SITE.url}${path === "/" ? "" : path}`

export const organization = () => ({
  "@type": "Organization",
  "@id": `${SITE.url}/#organization`,
  name: SITE.name,
  url: SITE.url,
  logo: `${SITE.url}/favicon.svg`,
  email: SITE.email,
  sameAs: [SITE.sourceUrl],
})

export const website = () => ({
  "@type": "WebSite",
  "@id": `${SITE.url}/#website`,
  name: SITE.name,
  url: SITE.url,
  description: SITE.description,
  publisher: { "@id": `${SITE.url}/#organization` },
  inLanguage: "en",
})

export const software = () => ({
  "@type": "SoftwareApplication",
  "@id": `${SITE.url}/#software`,
  name: SITE.name,
  description: SITE.description,
  applicationCategory: "BusinessApplication",
  applicationSubCategory: "App Store Optimization (ASO) keyword research tool",
  operatingSystem: "macOS, Windows",
  url: SITE.url,
  downloadUrl: url("/download"),
  codeRepository: SITE.sourceUrl,
  publisher: { "@id": `${SITE.url}/#organization` },
  featureList: [
    "App Store keyword research with Apple's popularity score",
    "Keyword difficulty from the apps that rank",
    "App Store rank tracking in every country",
    "App Store Connect listing editor",
    "ASO audit",
    "Screenshots and app preview videos",
    "Works with Claude Code, Codex, Gemini CLI and Ollama",
  ],
  offers: { "@type": "Offer", url: url("/pricing"), category: "subscription" },
})

/** The JSON-LD every page carries. */
export const siteGraph = () => ({ "@context": "https://schema.org", "@graph": [organization(), website(), software()] })

export function breadcrumbs(items: { name: string; path: string }[]) {
  return {
    "@type": "BreadcrumbList",
    itemListElement: items.map((item, i) => ({ "@type": "ListItem", position: i + 1, name: item.name, item: url(item.path) })),
  }
}

export function faqPage(faq: { q: string; a: string }[]) {
  return {
    "@type": "FAQPage",
    mainEntity: faq.map((f) => ({ "@type": "Question", name: f.q, acceptedAnswer: { "@type": "Answer", text: f.a } })),
  }
}

/** A guide: the article, its place in the site, and its questions. */
export function guideGraph(guide: Guide) {
  return {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "Article",
        "@id": `${url(guide.path)}#article`,
        headline: guide.h1,
        description: guide.description,
        datePublished: guide.updated,
        dateModified: guide.updated,
        mainEntityOfPage: url(guide.path),
        author: { "@id": `${SITE.url}/#organization` },
        publisher: { "@id": `${SITE.url}/#organization` },
        about: { "@id": `${SITE.url}/#software` },
        inLanguage: "en",
      },
      breadcrumbs([
        { name: SITE.name, path: "/" },
        { name: guide.h1, path: guide.path },
      ]),
      faqPage(guide.faq),
    ],
  }
}
