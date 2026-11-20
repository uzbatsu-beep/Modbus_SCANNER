# Repository governance & release approval (PR #9)

[Русская версия](RELEASE_GATES.ru.md) · [Windows QA](WINDOWS_QA.md) · [Field validation](FIELD_VALIDATION_v1.0.1.md)

## Source-of-truth and activation status

The JSON files in `.github/rulesets/` are **templates**, NOT active GitHub
repository rules until an administrator imports and activates them in Settings.
Workflow YAML cannot configure required reviewers for a GitHub Environment.
**Do not claim PR #9 is operationally complete until the live rulesets and
environment settings have been verified.** The manual release preflight
**fails closed** if the active rulesets are absent; the publishing job fails
closed without the provisioned environment secret.

## 1. Protect main (administrator action)

1. Open **Settings → Rules → Rulesets → New ruleset → Import a ruleset**
   and select `.github/rulesets/main.json`. Set enforcement **Active**.
2. Confirm the target is the default branch (`main`). Bypass list must be empty.
   Reject force-push and deletion. Require all changes via PR; one independent
   approving review, dismissal of stale reviews, approval of the last push,
   and conversation resolution.
3. Require up-to-date status checks (strict mode):
   - `Governance (release gate tests)`
   - `Frontend (typecheck + build)`
   - `Backend (cargo check + tests) (windows-latest)`
   - `Backend (cargo check + tests) (ubuntu-latest)`
   - `Backend (cargo check + tests) (macos-latest)`
4. Add a **second trusted GitHub contributor with repository write access** to
   review contributions from the project maintainer. GitHub does not treat
   an author's self-approval as an independent review. Requiring code-owner
   approval while the sole CODEOWNER authored the PR can deadlock the repo;
   the template deliberately requires an independent review instead.

Import the second template `.github/rulesets/release-tags.json` to protect
`refs/tags/v*` against **all updates** (including fast-forward moves) and
deletion. Tag creation is intentionally still permitted. Creation of
new tags remains allowed, because the protected Release job creates the tag
only after preflight and environment approval. A manually pushed tag by itself
does not trigger publishing (no `push.tags` trigger in `release.yml`).

**Verify in GitHub:** `main` is reported protected, both rulesets show
**Active**, a direct push to `main` is blocked, and a PR with missing CI/review
cannot merge. Do not perform force-push or tag deletion as a test.

## 2. Configure independent production-release approval

Open **Settings → Environments → New environment** and create
`production-release`.

- Configure **Required reviewers** (a trusted reviewer distinct from the
  person dispatching the workflow), and enable **Prevent self-review**.
- Restrict deployments to **selected branches**, `main` only.
- Create an **environment secret**, `OT_RELEASE_APPROVAL_GATE`, with a
  securely generated, non-empty random value. Never commit or display its value.
- Confirm the environment shows required reviewers and the approval step
  actually pauses the publication job.

The secret is a fail-closed provisioning sentinel: it does **not** by itself
prove human review. The actual review is enforced by GitHub's deployment
protection rules, which cannot be switched on via a repository commit.

## 3. Release preflight and acceptance evidence

Before starting a release:

1. All changes must be merged to `main` through approved PRs. The release
   workflow runs from **main only** and builds `github.sha` exactly.
2. CI must have a successful **push-to-main** workflow for that exact SHA,
   including all five checks. Green PR-only checks are insufficient.
3. Windows QA must have a successful `push` or `workflow_dispatch` run on
   that **same main SHA**, with all jobs passing and a non-expired versioned
   EXE/MSI artifact. QA now runs automatically on each push to main, and can
   also be dispatched from Actions.
4. Run an isolated physical Modbus RTU/ASCII/TCP bench test and record results
   in a **GitHub Issue** in this repository (NOT a PR comment). Include source
   SHA, relevant hardware and firmware, serial/network settings, test protocol,
   observations, limits, result (PASS/FAIL), date, reviewer and evidence.
   Do not test live process writes without an explicitly isolated test bench.
5. Ensure the tag `vX.Y.Z` matches all version manifests exactly and does not
   already exist. Publication is **new-only** (no overwrite).

Run **Actions → Release (manual, protected) → Run workflow** on `main`, entering
`tag`, `qa_run_id`, `bench_passed=true`, and the issue URL.

The release-gate script verifies exact SHA matching, live active Rulesets,
the five CI jobs, QA completion and artifact presence, tag nonexistence, and the
bench-acceptance assertion and evidence link. The last two are human
attestations; the script cannot determine hardware safety from an issue URL.

The Windows build generates EXE, MSI, `SHA256SUMS.txt`, and `BUILD-INFO.txt`.
The publishing job first waits for protected environment approval, checks the
sentinel secret, and **rechecks** the remote main SHA, Rulesets, CI/QA and evidence
before creating the official GitHub Release.

## 4. Failure handling

- **Rulesets absent / no reviewer:** release preflight blocks. Import and
  activate the templates; invite an independent reviewer.
- **QA SHA differs from main:** rerun Windows QA on current main, then supply
  the new run ID. Do not reuse a passing PR artifact for a different SHA.
- **Missing bench evidence:** use a QA-only artifact; do not publish a release.
- **Environment not configured:** publish job remains blocked/fails; create
  required reviewers, prevent self-review, selected branch and the secret.
- **Existing tag/release:** do not replace it. Use a new incremented version
  through the normal PR/version process.

## 5. Scope limits

This process raises the release bar, but repository administrators can still
alter GitHub settings and workflow files. GitHub Free/public repository support
for deployment environment reviewers applies; a second human reviewer is
required if self-approval is disabled. This does not certify safety under
IEC 62443 or replace testing on the relevant Modbus device and RS-485 gateway.
