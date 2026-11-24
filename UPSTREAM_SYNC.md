# Upstream synchronization

Stream Tube Desktop is maintained as a personal fork. Upstream updates are incorporated **only when explicitly requested**, after reviewing the incoming code. There is no automatic synchronization.

## Repository structure

- `origin`: https://github.com/Parlaxz/Stream-Tube-Desktop
- `upstream`: https://github.com/Avaneesh-Inamdar/Stream-Tube-Desktop
- `main`: clean mirror of upstream's last accepted `main` commit. No custom changes.
- `custom`: personal development branch and default branch of the fork.

## Procedure

### 1. Fetch and inspect

Start with a clean working tree. Do not discard uncommitted work; stop if necessary.

```bash
git status --short
git fetch origin
git fetch upstream
```

Review everything added upstream since the last accepted update:

```bash
git log --oneline main..upstream/main
git diff --stat main upstream/main
git diff main upstream/main
```

Fetching does not merge or execute upstream application code. Review incoming changes before building or running them. Pay particular attention to authentication, tokens and cookies, credential storage, network destinations, dependencies and lockfiles, build scripts, GitHub Actions, executable downloads, and the built-in updater. Investigate suspicious changes and stop if anything remains unexplained. Do not merge if the security review is inconclusive. Report findings to the owner.

### 2. Merge approved updates

Only proceed after the review passes. Ensure `main` and `custom` are up to date with their corresponding remote branches; stop on unexpected divergence. Use the reviewed upstream commit SHA in place of `REVIEWED_SHA` below so that only inspected code is merged.

```bash
git switch main
git merge --ff-only origin/main
git merge --ff-only REVIEWED_SHA
git push origin main

git switch custom
git merge --ff-only origin/custom
git merge --no-ff --no-commit main
```

If there are no incoming changes, skip the merge. Keep all personal modifications on `custom`.

### 3. Validate and push

Review the merged changes, run the existing relevant tests, and build and launch the application. Verify that custom features still work and that the updater still points at this fork's releases (`REPO` in `src/updater.rs`), never at upstream. If validation fails, do not push; fix the issue or abort the pending merge.

Once validated:

```bash
git commit -m "Merge reviewed upstream updates"
git push origin custom
```

Only commit if a merge is pending. If a push is rejected because of concurrent changes, stop and reconcile them without force-pushing.

## Merge conflicts

If Git reports a conflict, do not blindly accept either side. Inspect the affected files and preserve both the upstream improvements and personal features. If the correct resolution is unclear, report it and ask the owner before proceeding.

To cancel an unfinished merge:

```bash
git merge --abort
```

The remote `custom` branch remains unchanged until a successful push. The clean `main` mirror may already have advanced; the next attempt can merge it into `custom` again.

## Rules

- Never automatically sync, install, or execute upstream updates.
- Never force-push, discard local work, or push to the original repository.
- Keep the updater pointed at this fork's releases only.
- Keep synchronization simple; no additional infrastructure is required.

## Future agent instruction

> Audit the latest upstream changes and synchronize according to UPSTREAM_SYNC.md. Preserve our custom features, stop if anything suspicious or unresolved is found, and verify the merged application before pushing.
