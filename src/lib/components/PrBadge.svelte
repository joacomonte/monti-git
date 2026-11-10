<!-- A branch's pull request on GitHub, as a small chip: number, state by icon and color, and its checks. Click opens
     it in the browser. -->
<script lang="ts">
  import { CircleCheck, CircleX, Clock, GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft } from "@lucide/svelte";
  import { api, type PullRequest } from "$lib/api";

  let { pr, id }: { pr: PullRequest; id: string } = $props();
  const ICON = { open: GitPullRequest, draft: GitPullRequestDraft, merged: GitMerge, closed: GitPullRequestClosed };
  const STATE = { open: "Open", draft: "Draft", merged: "Merged", closed: "Closed" };
  const REVIEW = { approved: "approved", changes: "changes requested", required: "waiting for review" };
  const CHECKS = { pass: "checks passed", fail: "checks failed", pending: "checks running" };
  const title = $derived([`#${pr.number} ${pr.title}`, [STATE[pr.state], pr.review && REVIEW[pr.review], pr.checks && CHECKS[pr.checks]].filter(Boolean).join(" · "), "Click: open on GitHub"].join("\n"));
  const Icon = $derived(ICON[pr.state]);
</script>

<button class="pr {pr.state}" {id} {title} onclick={(e) => { e.stopPropagation(); api.openUrl(pr.url); }}>
  <Icon size={12} strokeWidth={2.5} />#{pr.number}
  {#if pr.checks === "pass"}<CircleCheck size={11} strokeWidth={2.75} class="text-green-500" />
  {:else if pr.checks === "fail"}<CircleX size={11} strokeWidth={2.75} class="text-red-500" />
  {:else if pr.checks === "pending"}<Clock size={11} strokeWidth={2.75} class="text-amber-400" />{/if}
  {#if pr.review === "approved"}<span class="review ok">Approved</span>{:else if pr.review === "changes"}<span class="review bad">Changes</span>{/if}
</button>

<style>
  .pr { display: inline-flex; flex-shrink: 0; align-items: center; gap: 4px; height: 22px; padding: 0 7px; border-radius: 7px; font-size: 11.5px; font-weight: 600;
    color: var(--c); background: color-mix(in oklab, var(--c) 13%, transparent); }
  .pr:hover { background: color-mix(in oklab, var(--c) 22%, transparent); }
  .open { --c: #34c77b; }
  .draft { --c: #8b93a1; }
  .merged { --c: #a78bfa; }
  .closed { --c: #f43f5e; }
  .review { font-size: 10.5px; font-weight: 600; }
  .review.ok { color: #34c77b; }
  .review.bad { color: #f43f5e; }
</style>
