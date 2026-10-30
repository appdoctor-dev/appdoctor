import { createFileRoute } from "@tanstack/react-router"
import { GuidePage, guideHead } from "#/components/guide"
import { KEYWORD_TOOLS } from "#/content/guides"
import { guideGraph } from "#/lib/jsonld"

export const Route = createFileRoute("/aso-keyword-research-tools")({
  head: () => ({
    meta: guideHead(KEYWORD_TOOLS),
    scripts: [{ type: "application/ld+json", children: JSON.stringify(guideGraph(KEYWORD_TOOLS)) }],
  }),
  component: () => <GuidePage guide={KEYWORD_TOOLS} />,
})
