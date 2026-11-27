<script lang="ts">
  import { onMount } from "svelte";
  import { api, type OrderTargetDto } from "../api";
  import ContextMenu from "./ContextMenu.svelte";
  import type { ContextItem } from "./context";
  import { instrumentMenuItems } from "./instrumentMenu";

  interface Props {
    x: number;
    y: number;
    instrumentKey: string;
    symbol?: string;
    extras?: ContextItem[];
    onbuy: (key: string) => void;
    onsell: (key: string) => void;
    onchain: (underlying: string) => void;
    onclose: () => void;
  }
  let {
    x,
    y,
    instrumentKey,
    symbol = "",
    extras = [],
    onbuy,
    onsell,
    onchain,
    onclose,
  }: Props = $props();

  let target = $state<OrderTargetDto | null>(null);
  let loaded = $state(false);

  onMount(() => {
    let cancelled = false;
    void api
      .orderTarget(instrumentKey)
      .then((result) => {
        if (!cancelled) target = result;
      })
      .catch(() => {
        if (!cancelled) target = null;
      })
      .finally(() => {
        if (!cancelled) loaded = true;
      });
    return () => {
      cancelled = true;
    };
  });

  const items = $derived(
    instrumentMenuItems(target, loaded, instrumentKey, symbol, extras, {
      onbuy,
      onsell,
      onchain,
    }),
  );
</script>

<ContextMenu {x} {y} items={items} {onclose} />
