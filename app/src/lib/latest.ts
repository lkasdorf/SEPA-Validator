/** Tracks the newest of several overlapping async requests, so a slow, superseded
 *  request can drop its result instead of overwriting a newer one. */
export function createLatest() {
  let current = 0;
  return {
    /** Start a request; keep the token and check it after every await. */
    begin(): number {
      return ++current;
    },
    isCurrent(token: number): boolean {
      return token === current;
    },
  };
}
