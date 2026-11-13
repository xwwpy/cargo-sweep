/** Compare the release AND compiler commit, not just the nightly release number. */
export function sameRustBuild(
  actual: string | null | undefined,
  expected: string | null | undefined,
): boolean {
  if (!actual || !expected) return false;
  const parse = (value: string) => {
    const normalized = value.trim().replace(/^rustc\s+/i, "");
    return {
      release: normalized.split(/\s+/)[0],
      commit: normalized.match(/\(([0-9a-f]{7,})(?:\s|\))/i)?.[1]?.toLowerCase(),
    };
  };
  const left = parse(actual);
  const right = parse(expected);
  if (!left.release || left.release !== right.release) return false;
  if (right.commit) {
    return Boolean(left.commit && (
      left.commit.startsWith(right.commit) || right.commit.startsWith(left.commit)
    ));
  }
  // Without a commit, a nightly version cannot establish that it is up to date.
  return !right.release.includes("nightly");
}
