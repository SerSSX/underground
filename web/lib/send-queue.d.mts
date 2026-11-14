export interface SendQueue<T> {
  enqueue(item: T): void;
  readonly busy: boolean;
  readonly waiting: number;
}
export function createSendQueue<T, R>(h: {
  send: (item: T) => Promise<R>;
  onSent?: (item: T, result: R) => void;
  onFailed?: (failed: T[], error: unknown) => void;
}): SendQueue<T>;
export function stashFailed(
  drafts: Map<string, string>,
  failed: { peerId: string; body: string }[],
  openPeerId: string | null | undefined,
): string;
export function takeDraft(drafts: Map<string, string>, peerId: string): string;
