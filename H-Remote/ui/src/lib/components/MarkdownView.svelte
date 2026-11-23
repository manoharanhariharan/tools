<script lang="ts">
  interface Props {
    markdown: string;
  }
  let { markdown }: Props = $props();

  function escapeHtml(value: string): string {
    return value
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  const html = $derived.by(() => {
    const escaped = escapeHtml(markdown);
    const lines = escaped.split(/\r?\n/);
    const output: string[] = [];
    let inCode = false;
    let listOpen = false;
    for (const line of lines) {
      if (line.trim().startsWith("```")) {
        if (inCode) {
          output.push("</code></pre>");
          inCode = false;
        } else {
          if (listOpen) {
            output.push("</ul>");
            listOpen = false;
          }
          output.push("<pre><code>");
          inCode = true;
        }
        continue;
      }
      if (inCode) {
        output.push(`${line}\n`);
        continue;
      }
      if (/^\s*[-*]\s+/.test(line)) {
        if (!listOpen) {
          output.push("<ul>");
          listOpen = true;
        }
        output.push(`<li>${inline(line.replace(/^\s*[-*]\s+/, ""))}</li>`);
        continue;
      }
      if (listOpen) {
        output.push("</ul>");
        listOpen = false;
      }
      if (line.startsWith("### ")) output.push(`<h3>${inline(line.slice(4))}</h3>`);
      else if (line.startsWith("## ")) output.push(`<h2>${inline(line.slice(3))}</h2>`);
      else if (line.startsWith("# ")) output.push(`<h1>${inline(line.slice(2))}</h1>`);
      else if (line.trim() === "") output.push("");
      else output.push(`<p>${inline(line)}</p>`);
    }
    if (listOpen) output.push("</ul>");
    if (inCode) output.push("</code></pre>");
    return output.join("\n");
  });

  function inline(value: string): string {
    return value
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  }
</script>

<div class="markdown">{@html html}</div>

<style>
  .markdown {
    font-size: var(--hr-font-size-small);
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3) {
    margin: 10px 0 6px;
  }

  .markdown :global(p) {
    margin: 0 0 8px;
  }

  .markdown :global(ul) {
    margin: 0 0 8px 18px;
    padding: 0;
  }

  .markdown :global(pre) {
    background: color-mix(in srgb, #000 28%, var(--hr-primary-dark));
    border-radius: var(--hr-radius-small);
    padding: 9px;
    overflow: auto;
  }

  .markdown :global(code) {
    background: color-mix(in srgb, var(--hr-primary-text) 12%, transparent);
    border-radius: 3px;
    padding: 1px 4px;
  }

  .markdown :global(a) {
    color: var(--hr-accent-light);
  }
</style>
