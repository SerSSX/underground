"use client";

import type { ReactNode } from "react";

interface Props {
  /** The <tr> of <th> headers. Give the trailing actions <th> className="actions". */
  headers: ReactNode;
  /** The <tr> data rows (rendered only in the loaded, non-empty state). */
  children: ReactNode;
  /** Column count, for state rows spanning the whole table. */
  colSpan: number;
  loading?: boolean;
  error?: string | null;
  empty?: boolean;
  emptyText?: string;
  emptyIcon?: string;
  onRetry?: () => void;
  toolbar?: ReactNode;
}

/**
 * One data-table primitive for every admin queue. Guarantees the actions column is never
 * clipped (overflow-x wrapper + sticky-right column via the `.actions` class) and gives
 * consistent skeleton-loading / empty / error-with-retry states.
 */
export default function DataTable({
  headers,
  children,
  colSpan,
  loading,
  error,
  empty,
  emptyText = "Нічого не знайдено.",
  emptyIcon = "∅",
  onRetry,
  toolbar,
}: Props) {
  return (
    <>
      {toolbar && <div className="table-toolbar">{toolbar}</div>}
      <div className="tablewrap">
        <table className="table">
          <thead>{headers}</thead>
          <tbody>
            {loading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <tr key={i}>
                  {Array.from({ length: colSpan }).map((__, j) => (
                    <td key={j}><span className="skel" style={{ width: j === 0 ? "70%" : "50%" }} /></td>
                  ))}
                </tr>
              ))
            ) : error ? (
              <tr>
                <td colSpan={colSpan}>
                  <div className="table-state">
                    <span className="ico" aria-hidden>⚠</span>
                    <div>{error}</div>
                    {onRetry && <button className="btn ghost sm" style={{ marginTop: 12 }} onClick={onRetry}>Спробувати ще</button>}
                  </div>
                </td>
              </tr>
            ) : empty ? (
              <tr>
                <td colSpan={colSpan}>
                  <div className="table-state"><span className="ico" aria-hidden>{emptyIcon}</span><div>{emptyText}</div></div>
                </td>
              </tr>
            ) : (
              children
            )}
          </tbody>
        </table>
      </div>
    </>
  );
}
