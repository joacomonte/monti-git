<!-- A short real-life example under an option's description, for choices that may not be obvious. It unfolds while the
     option (the button or switch around it) is hovered or focused. Use it inside the option's text column. -->
<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<span class="example"><span class="fold"><span class="box">e.g. {text}</span></span></span>

<style>
  .example { display: grid; grid-template-rows: 0fr; transition: grid-template-rows 0.25s cubic-bezier(0.22, 1, 0.36, 1); }
  .fold { min-height: 0; overflow: hidden; }
  .box {
    display: block; margin-top: 6px; padding: 6px 9px; border-radius: 7px; text-align: left; white-space: normal;
    font-size: 11.5px; font-weight: 400; font-style: italic; line-height: 1.45; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08);
  }
  :global(:is(button, [role="switch"]):is(:hover, :focus-visible)) .example { grid-template-rows: 1fr; }
  @media (prefers-reduced-motion: reduce) { .example { transition: none; } }
</style>
