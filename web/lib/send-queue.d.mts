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
