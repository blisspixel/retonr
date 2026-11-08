# Repository Instructions

## Authorship and style

- Do not add generated-by, co-authored-by, or similar attribution to commits, files, documentation, comments, or release notes.
- Do not use emojis in tracked repository content.
- Do not use en dash or em dash characters. Use a hyphen, colon, semicolon, or parentheses instead.
- Keep user-facing language direct, precise, and professional.

## Quality

- Run formatting, linting, tests, and repository policy checks before considering work complete.
- Maintain at least 80 percent line coverage for implemented Rust code.
- Treat warnings as errors in continuous integration.
- Fix relevant defects found while working instead of only documenting them.
- Keep continuous integration passing on every supported platform.

## Engineering

- Preserve the local-first and offline-after-setup product constraints.
- Prefer small, explicit interfaces and deterministic validation at trust boundaries.
- Do not claim that probabilistic semantic checks provide formal guarantees.
- Add regression fixtures for every fixed fidelity or structure-preservation defect.

## Start tidy, stop tidy

Work in this repository may come from several agent tools, and a session can end
partway through a task.

- Check first. Before starting new work, run `git fetch --prune` and check
  `git worktree list`, local and remote branches, and open pull requests.
  Finish, merge, or park leftover work instead of starting a parallel attempt.
  Squash merges hide merged work from ancestry, so check pull request state, not
  only `git branch --merged`.
- Checkpoint and hand off. Commit and push at meaningful checkpoints. Before
  stopping, leave a short handoff in the pull request or commit message saying
  what is done and what is next.
- Keep the tree clean. Test fixtures, scratch scripts, logs, and generated
  output go only in designated test or temporary locations, or in `/scratch/`,
  which is gitignored. Leave throwaway files out of the rest of the tree, and
  remove scratch artifacts before finishing.
- Keep branches short-lived. Merge promptly and delete a branch after its work
  has landed. Keep GitHub current: push finished work, update or close pull
  requests, and close finished issues.
- Goal state. Leave one `main` whose latest quality run is passing, with
  finished work on `main`.
- Remove only with evidence. Remove a worktree or branch only once its work is
  merged, contained in `main`, or pushed to the remote. Never use `--force` to
  remove one. Report anything uncertain instead of deleting it. Run
  `git worktree prune` for entries whose directories are gone.
- Ask when unclear. If it is unclear whether leftover work should be finished,
  kept, or removed, stop and ask.
