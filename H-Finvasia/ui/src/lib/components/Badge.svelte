<script lang="ts">
  interface Props {
    label: string;
    tone?: "neutral" | "ok" | "warn" | "err";
    dot?: boolean;
    title?: string;
  }
  let { label, tone = "neutral", dot = false, title = "" }: Props = $props();
</script>

<span class="badge {tone === "neutral" ? "" : tone}" {title}>
  {#if dot}<span class="dot"></span>{/if}{label}
</span>
