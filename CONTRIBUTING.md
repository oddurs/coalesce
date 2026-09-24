# Contributing

Run `scripts/setup` once. It wires the git hooks and installs dependencies.

## Workflow

1. `scripts/agent start <type>/<slug>` creates a branch and a worktree under
   `../.worktrees/black-hole/<branch>/`. Work there. Never in the primary checkout.
2. `scripts/agent check` runs format, lint (warnings denied), tests, build.
3. `scripts/agent commit "<type>(<scope>): <summary>"` — Conventional Commits, subject
   at most 72 characters, no trailing period, body says why.
4. `scripts/agent pr` pushes and opens the pull request. Fill in the template honestly:
   say where the change is weak.
5. After the squash merge, `scripts/agent done` removes the worktree and branch.

`main` advances only through a merged pull request. The hooks and branch protection
enforce this. Required approvals are set to 0 because this is a solo project.
