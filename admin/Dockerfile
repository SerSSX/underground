# syntax=docker/dockerfile:1
# Multi-stage build for the Underground admin client (Next.js, standalone output).
# NEXT_PUBLIC_* is inlined at build time, so the public URLs arrive as build args: one image per environment
# URL set (a known gap against "one artifact, every environment", to close before prod — docs/STATE.md).

FROM node:22-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-slim AS builder
WORKDIR /app
ARG NEXT_PUBLIC_API_BASE_URL
ARG NEXT_PUBLIC_SITE_URL
ARG UG_BUILD_SHA
ENV NEXT_PUBLIC_API_BASE_URL=$NEXT_PUBLIC_API_BASE_URL
ENV NEXT_PUBLIC_SITE_URL=$NEXT_PUBLIC_SITE_URL
ENV UG_BUILD_SHA=$UG_BUILD_SHA
ENV NEXT_TELEMETRY_DISABLED=1
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build

FROM node:22-slim AS runtime
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME=0.0.0.0
WORKDIR /app
COPY --from=builder --chown=node:node /app/.next/standalone ./
COPY --from=builder --chown=node:node /app/.next/static ./.next/static
# Source files must never ship: on 2026-11-12 file tracing put app/page.tsx and app/layout.tsx into the
# standalone output, and the server took that app/ folder for the router root (DEV served the wrong page with
# no layout). The runtime needs only .next/, server.js and node_modules.
RUN rm -rf ./app ./src ./pages
USER node
EXPOSE 3000
HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=5 \
    CMD node -e "fetch('http://127.0.0.1:'+process.env.PORT+'/').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["node", "server.js"]
