export interface ChainTabLike {
  id: string;
  loaded: boolean;
  pinned: boolean;
  pending: unknown | null;
}

export function isBlankChainTab(tab: ChainTabLike): boolean {
  return !tab.loaded && !tab.pinned && tab.pending === null;
}

/**
 * Keep exactly one reusable blank chain tab. Pinning calls this without
 * changing the active tab: the pinned chain stays focused and the spare blank
 * tab is just made available for the next underlying.
 */
export function normaliseBlankChainTabs<T extends ChainTabLike>(
  tabs: readonly T[],
  create: () => T,
): T[] {
  const blanks = tabs.filter(isBlankChainTab);
  if (blanks.length === 0) return [...tabs, create()];
  if (blanks.length === 1) return [...tabs];
  const keep = blanks[0].id;
  return tabs.filter((tab) => !isBlankChainTab(tab) || tab.id === keep);
}
