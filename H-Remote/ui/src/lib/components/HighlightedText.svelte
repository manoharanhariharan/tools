<script lang="ts">
  import type { TextRange } from "../ipc/contracts";

  interface Props {
    text: string;
    ranges: TextRange[];
  }
  let { text, ranges }: Props = $props();

  const segments = $derived.by(() => {
    const characters = Array.from(text);
    const sorted = [...ranges]
      .filter((range) => range.end > range.start && range.start < characters.length)
      .sort((left, right) => left.start - right.start);
    const result: Array<{ text: string; marked: boolean }> = [];
    let cursor = 0;
    for (const range of sorted) {
      const start = Math.max(cursor, range.start);
      const end = Math.min(characters.length, range.end);
      if (start > cursor) {
        result.push({ text: characters.slice(cursor, start).join(""), marked: false });
      }
      if (end > start) {
        result.push({ text: characters.slice(start, end).join(""), marked: true });
      }
      cursor = Math.max(cursor, end);
    }
    if (cursor < characters.length) {
      result.push({ text: characters.slice(cursor).join(""), marked: false });
    }
    return result;
  });
</script>

{#each segments as segment, index (index)}
  {#if segment.marked}<mark>{segment.text}</mark>{:else}{segment.text}{/if}
{/each}

<style>
  mark {
    background: color-mix(in srgb, var(--hr-accent-mid) 38%, transparent);
    color: inherit;
    border-radius: 2px;
    padding: 0 1px;
  }
</style>
