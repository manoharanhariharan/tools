<script lang="ts">
  interface Props {
    size?: number;
  }
  let { size = 13 }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  aria-hidden="true"
  focusable="false"
>
  <path
    d="M2.8 6.4A1.6 1.6 0 0 1 4.4 4.8h4.1c.43 0 .84.17 1.14.47L11.3 7h8.3a1.6 1.6 0 0 1 1.6 1.6v8.6a1.6 1.6 0 0 1-1.6 1.6H4.4a1.6 1.6 0 0 1-1.6-1.6z"
    fill="currentColor"
  />
  <path
    d="M4.2 8.6h15.6v2.1H4.2z"
    fill="#fff"
    opacity="0.16"
  />
</svg>

<style>
  svg {
    display: block;
  }
</style>
