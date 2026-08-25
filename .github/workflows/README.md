# CI/Actions notes for this fork

This file collects debugging notes and gotchas discovered while getting
Android APK builds to actually show up as downloads (run artifacts and/or
GitHub Releases) on this fork. Read this before touching any workflow file
under `.github/workflows/`.

## TL;DR: where do downloads show up?

- **Run artifacts** (always, once `upload-artifact: true` for that run):
  open the specific workflow run → scroll to the **Artifacts** section at the
  bottom. This works regardless of `GITHUB_TOKEN` permissions, because
  `actions/upload-artifact` doesn't need `contents` scope.
- **Releases page** (`/releases`, linked from the repo homepage): only
  populated if the `Publish signed/unsigned apk package` (or the equivalent
  `Publish Release` step for other platforms) succeeds. That step uses
  `softprops/action-gh-release`, which needs a **write-scoped** `GITHUB_TOKEN`
  (`contents: write`).

Android APKs land at `rustdesk-<version>-{aarch64,armv7,x86_64,universal}.apk`
in both places once `contents: write` is granted (see below).

## Gotcha 1: pushing to `develop`/`master` doesn't always trigger CI

`ci.yml` and `flutter-ci.yml` used to have `paths-ignore: [".github/**", ...]`
on their `push` trigger. That means a commit that **only** touches files
under `.github/**` (e.g. a workflow-only fix) does **not** trigger CI on
push — so you can silently ship a broken/untested workflow change and never
find out until the next unrelated commit.

We removed `.github/**` from that `paths-ignore` list so workflow-only edits
are validated too. Two things to remember:

1. GitHub evaluates `push` path filters using the workflow file **as it
   exists in the pushed commit** (i.e. the new version), not the version
   before the push. This means a commit that both changes a workflow trigger
   *and* removes its own exclusion for that same push can be
   self-triggering — verified empirically in this repo (a commit that only
   touched `.github/workflows/*.yml`, including removing the `.github/**`
   ignore, did trigger a new run for that same push).
2. Before that removal, if you need to test a workflow-only change and can't
   rely on the above trick, bundle the workflow edit with **some** real,
   non-`.github/**` change in the same push (there's no other way to force a
   push-triggered run without `workflow_dispatch` access — see Gotcha 4).

## Gotcha 2: reusable workflow permissions can only be downgraded, never elevated

`flutter-ci.yml` / `flutter-nightly.yml` / `flutter-tag.yml` all call the
reusable `flutter-build.yml` via `uses: ./.github/workflows/flutter-build.yml`.
Per GitHub's docs
([reusing-workflow-configurations](https://docs.github.com/en/actions/reference/workflows-and-actions/reusing-workflow-configurations)):

> If `jobs.<job_id>.permissions` is not specified in the calling job, the
> called workflow will have the default permissions for the `GITHUB_TOKEN`.
> The `GITHUB_TOKEN` permissions passed from the caller workflow can be only
> downgraded (not elevated) by the called workflow.

This repo's default `GITHUB_TOKEN` permission is read-only (Settings →
Actions → General → Workflow permissions). That's why every
`softprops/action-gh-release` step (used for Windows/macOS/Linux/Android
release publishing) failed with:

```
👩‍🏭 Creating new GitHub release for tag nightly...
⚠️ GitHub release failed with status: 403
##[error]Too many retries.
```

...even though the actual build steps succeeded. The fix requires declaring
`contents: write` in **both** places:

- The calling job (`permissions: contents: write` on the `run-ci` /
  `run-flutter-*-build` job in `flutter-ci.yml`, `flutter-nightly.yml`,
  `flutter-tag.yml`), which raises the ceiling available to grant downstream.
- The reusable workflow itself (top-level `permissions: contents: write` in
  `flutter-build.yml`), which actually requests/uses that scope.

An explicit `permissions:` block in a workflow **does** override the repo's
"read-only default" setting — that repo setting is only the fallback used
when no workflow declares permissions explicitly. It is not a hard ceiling
you need admin access to lift.

## Gotcha 3: don't let a release-publish failure fail the whole job

Even with correct permissions, treat "attach the apk as a run artifact" and
"publish it to a GitHub Release" as two independent steps:

- `Upload unsigned APK to Artifacts` (per-arch and universal Android jobs)
  always runs when `UPLOAD_ARTIFACT == 'true'` and no signing key is
  configured, uploading the debug-signed apk from `signed-apk/` via
  `actions/upload-artifact`. This never depends on `contents` permissions.
- `Publish signed/unsigned apk package` (the `softprops/action-gh-release`
  step) is marked `continue-on-error: true`. If release publishing ever
  breaks again (permissions regressed, rate limits, etc.), the job still
  succeeds and the apk is still downloadable from Artifacts — you just won't
  get a Releases entry until it's fixed.

Apply the same pattern to any other platform's publish step if it starts
failing for a similar reason.

## Gotcha 4: the cloud-agent's `gh`/API token can't do everything

While debugging this, several `gh`/API calls returned
`HTTP 403: Resource not accessible by integration`, specifically:

- `gh workflow run <file> --ref <branch>` (workflow_dispatch)
- `gh run cancel <run-id>`
- `gh api repos/.../actions/permissions` (reading the Workflow permissions
  setting)

These need a repo-admin-scoped token / the actual repo owner clicking
through the GitHub UI — an automated agent working through this bot
integration cannot dispatch workflows, cancel runs, or introspect/change
Actions settings via the API. Don't burn time retrying these; instead:

- To test a workflow change: push a commit to a branch that the trigger's
  `on:` config actually matches (see Gotcha 1).
- To stop wasting CI minutes on a run you know is stale: just let it finish;
  you can't cancel it from here.
- To check/change the repo's default `GITHUB_TOKEN` permission: ask a human
  with admin access to check Settings → Actions → General → Workflow
  permissions, or just declare `permissions:` explicitly in the workflow
  (Gotcha 2) so it doesn't matter what the default is.

## Gotcha 5: concurrency/queueing makes runs slow and easy to misread

`flutter-build.yml` fans out into ~20 jobs across Windows/macOS/Linux/
Android/iOS runners. Free-tier GitHub-hosted runner concurrency limits
(lower for macOS) mean:

- If a previous run on the same repo is still in flight, a newer run's jobs
  can sit in `queued` for 10-20+ minutes before a runner picks them up, even
  though nothing is actually broken.
- The Android apk build itself (bridge + NDK + Rust + Gradle) takes roughly
  25-30 minutes per architecture once a runner is assigned, so budget
  ~30-90 minutes end-to-end for `build rustdesk android apk *` jobs
  depending on queue pressure.
- Prefer polling specific jobs with `gh run view <run-id> --json jobs` (or
  the GitHub API) over `gh run watch`, which can itself exit with a non-zero
  status on a transient hiccup while the underlying run keeps going fine —
  don't treat a `gh run watch` failure as the workflow run having failed;
  re-check via `gh run view`.
