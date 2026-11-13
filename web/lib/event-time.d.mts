export function isOver(startsAt: string | null | undefined, endsAt?: string | null, now?: number): boolean;
export const LIVE_TICKET_STATUSES: string[];
export function splitTicketGroups<G extends { startsAt: string | null; endsAt: string | null; tickets: { status: string }[] }>(
  groups: G[],
  now?: number,
): { upcoming: G[]; past: G[] };
