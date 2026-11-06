// The prompt that asks your AI agent for a commit message: for a new commit, for amending the last one (its message
// rewritten to cover both), or for squashing the branch into one. Used by the commit box's "With AI" steps.

/** Where the changes go. */
export type AiMode = "new" | "amend" | "squash";
/** How long the description. */
export type AiLength = "concise" | "medium" | "long";

const LENGTHS: Record<AiLength, string> = {
  concise: "one or two short lines: what changed and why.",
  medium: "a short paragraph or 3–5 bullets: what changed and why, and any notable decision.",
  long: "complete: the problem or context, what changed by area, decisions and trade-offs, how to test it, and anything risky.",
};

/** The three lengths as choices: name and what you get. */
export const LENGTH_CHOICES: { key: AiLength; label: string; detail: string }[] = [
  { key: "concise", label: "Concise", detail: "Title plus one or two lines" },
  { key: "medium", label: "Medium", detail: "Title plus a short paragraph or a few bullets" },
  { key: "long", label: "Long", detail: "Context, changes by area, decisions, how to test, risks" },
];

export function aiPrompt({ repo, branch, base, mode, length }: { repo: string; branch: string; base: string | null; mode: AiMode; length: AiLength }) {
  const task = {
    new: ["Write a commit message for my uncommitted changes."],
    amend: [
      "Amend my uncommitted changes into the last commit (HEAD), so it stays one commit.",
      "- The message must cover both: read the last commit's message (`git log -1 --format=%B`) and the new changes, and rewrite title and/or body as a summary of all of it.",
    ],
    squash: [
      `Squash ${branch}'s own commits since it split from ${base} (\`git merge-base ${base} HEAD\`) plus my uncommitted changes into one single commit (e.g. \`git reset --soft <that merge-base>\` then commit).`,
      "- The message must cover everything: read those commits' messages (`git log --format=%B <merge-base>..HEAD`) and the new changes, and write one title and body that summarize all of it.",
    ],
  }[mode];
  return [
    `Repo: ${repo}`,
    `Branch: ${branch}`,
    ...task,
    "- Read the changes: if something is staged (`git diff --staged`), only that goes in; otherwise everything in `git status` and `git diff`, new files included.",
    "- Title: follow this repo's conventions (look at `git log --oneline -20`: e.g. `feat(scope): …`, a ticket id like MOA-123 from the branch name). Imperative, under ~72 characters.",
    `- Description: ${LENGTHS[length]}`,
    "- Same language as the recent commit messages.",
    mode === "new"
      ? "Give me just the message in a code block. Don't commit."
      : "Show me the new message and wait for my OK before rewriting anything. If it's already on origin, it'll need --force-with-lease: don't push, I'll do it myself.",
  ].join("\n");
}
