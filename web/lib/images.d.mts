export const MAX_SIDE: number;
export const SHRINK_OVER_BYTES: number;
export function smallVariant(url: string | null | undefined): string | null;
export function shrinkPlan(f: { size: number; type: string; width: number; height: number }): { width: number; height: number } | null;
