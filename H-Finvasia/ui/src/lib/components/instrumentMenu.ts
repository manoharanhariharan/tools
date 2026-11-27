import type { OrderTargetDto } from "../api";
import type { ContextItem } from "./context";

export interface InstrumentMenuHandlers {
  onbuy: (key: string) => void;
  onsell: (key: string) => void;
  onchain: (underlying: string) => void;
}

/**
 * One menu shape for every instrument row: tradeable rows get F1/F2, F&O rows
 * get the option chain, and the caller's row actions are appended last.
 */
export function instrumentMenuItems(
  target: OrderTargetDto | null,
  loaded: boolean,
  instrumentKey: string,
  fallbackSymbol: string,
  extras: ContextItem[],
  handlers: InstrumentMenuHandlers,
): ContextItem[] {
  if (!loaded) return [{ label: "Loading instrument…", disabled: true }];
  const list: ContextItem[] = [];
  if (!target) {
    list.push({ label: "Instrument metadata unavailable", disabled: true });
  } else {
    const label = target.trading_symbol || fallbackSymbol || instrumentKey;
    if (target.tradeable) {
      list.push({ label: `F1 — Buy ${label}`, action: () => handlers.onbuy(instrumentKey) });
      list.push({ label: `F2 — Sell ${label}`, action: () => handlers.onsell(instrumentKey) });
    } else {
      list.push({ label: `${label} is not tradeable`, disabled: true });
    }
    if (target.has_fno && target.underlying) {
      const underlying = target.underlying;
      list.push({
        label: `Open option chain (${underlying})`,
        action: () => handlers.onchain(underlying),
      });
    }
  }
  list.push(...extras);
  return list;
}
