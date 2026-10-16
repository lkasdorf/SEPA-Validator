/** Rows to render for a fixed-height list: the ones inside the viewport plus
 *  `overscan` on each side, clamped to `[0, total]`. `end` is exclusive. */
export function visibleRange(
  scrollTop: number,
  viewHeight: number,
  rowHeight: number,
  total: number,
  overscan = 20
): { start: number; end: number } {
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const last = Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewHeight)) / rowHeight);
  return {
    start: Math.min(total, Math.max(0, first - overscan)),
    end: Math.min(total, last + overscan),
  };
}
