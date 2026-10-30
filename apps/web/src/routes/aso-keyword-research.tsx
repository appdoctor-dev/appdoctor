import { createFileRoute } from "@tanstack/react-router"
import { GuidePage, guideHead } from "#/components/guide"
import { KEYWORD_RESEARCH } from "#/content/guides"
import { guideGraph } from "#/lib/jsonld"

export const Route = createFileRoute("/aso-keyword-research")({
  head: () => ({
    meta: guideHead(KEYWORD_RESEARCH),
    scripts: [{ type: "application/ld+json", children: JSON.stringify(guideGraph(KEYWORD_RESEARCH)) }],
  }),
  component: () => <GuidePage guide={KEYWORD_RESEARCH} />,
})
