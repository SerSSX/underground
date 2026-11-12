import { jsonLdString } from "../../lib/jsonld.mjs";

/** Renders a schema.org JSON-LD block (server component). The serializer escapes "<" so user-written text
 *  (event titles) can never close the script element — see lib/jsonld.mjs. */
export default function JsonLd({ data }: { data: Record<string, unknown> }) {
  return <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLdString(data) }} />;
}
