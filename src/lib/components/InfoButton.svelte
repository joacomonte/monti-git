<!-- "?" next to a label: hover for a tooltip, click for the full explanation. -->
<script lang="ts">
  import { CircleQuestionMark } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";

  let { title, text, id }: { title: string; text: string; id: string } = $props();
</script>

<Popover.Root>
  <Popover.Trigger {id} title={text} class="inline-flex shrink-0 text-tertiary hover:text-muted-foreground data-[state=open]:text-muted-foreground">
    <CircleQuestionMark size={10} />
  </Popover.Trigger>
  <Popover.Content class="w-[300px] gap-1.5 p-3.5">
    <div class="text-[12px] font-semibold">{title}</div>
    <div class="text-[12px] text-muted-foreground">{text}</div>
  </Popover.Content>
</Popover.Root>
