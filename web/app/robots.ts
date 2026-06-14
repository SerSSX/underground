import type { MetadataRoute } from "next";

const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** Crawlers may index public discovery pages; keep private/account routes out. */
export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: "*", allow: "/", disallow: ["/me/", "/organizer/", "/login", "/register"] },
    sitemap: `${SITE}/sitemap.xml`,
  };
}
