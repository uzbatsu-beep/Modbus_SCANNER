# Repository governance & manual release (solo maintainer)

[Русская версия](RELEASE_GATES.ru.md) · [Windows QA](WINDOWS_QA.md) · [Field validation](FIELD_VALIDATION_v1.0.1.md)

## Policy

The maintainer controls merges and official releases **without requiring a second person**.
Technical safeguards stay strict: PR-only changes, five required check contexts (two intentionally skipped on routine CI),
up-to-date `main`, protected `v*` tags, successful Windows QA on the **exact**
release commit, verifiable physical bench PASS, and a **manual** release dispatch.
No `push.tags` trigger, telemetry, automated release after merge, or bypass list.

The files `.github/rulesets/*.json` are importable templates, not automatically
applied GitHub settings. The existing main Ruleset was activated with one
approval under PR #9 and **must be edited in GitHub Settings** before merging
PR #10. The connected GitHub integration cannot change Rulesets itself.

## 1. Edit the ACTIVE main Ruleset

Open [main Ruleset](https://github.com/uzbatsu-beep/Modbus_SCANNER/rules/27728708)
(Settings → Rules → Rulesets). Edit the existing rule, **do not create a second
competing rule**. In *Require a pull request before merging*:

- Keep **Require a pull request before merging** enabled.
- Set **Required approvals = 0**.
- Disable **Require approval of the most recent reviewable push**.
- Disable **Require review from Code Owners**.
- Disable **Require extra approval for unattributed changes**, if shown.
- You may disable stale-approval dismissal because no approval is required.
- Keep **Require conversation resolution before merging**.

Do **not** change: enforcement **Active**, main/default-branch target, empty
bypass list, deletion/force-push restrictions, strict up-to-date status checks,
and the five required jobs:

```text
Governance (release gate tests)
Frontend (typecheck + build)
Backend (cargo check + tests) (windows-latest)
Backend (cargo check + tests) (ubuntu-latest)
Backend (cargo check + tests) (macos-latest)
```

Check the [release tag Ruleset](https://github.com/uzbatsu-beep/Modbus_SCANNER/rules/27728718):
keep it **Active**, with deletion, update and non-fast-forward denied for
`refs/tags/v*`. Do not change this rule.

After saving, verify `main` remains protected and the PR is mergeable **only
after passing CI**, without a separate approving reviewer. If the user-facing
GitHub Ruleset editor lacks one of the settings, use the template in
`.github/rulesets/main.json` after PR #10 is merged, or edit the values
manually. Never temporarily disable branch protection or use bypass.

## 2. No approval Environment

The Release workflow no longer references `production-release` or the
`OT_RELEASE_APPROVAL_GATE` secret. You do **not** need to create this
Environment, configure a required reviewer, or add an environment secret.
If you previously created the Environment, it is no longer used by the workflow;
its presence has no influence on publication.

Release is intentionally initiated once by the maintainer using
**Actions → Release (manual, verified) → Run workflow**. All three jobs
(preflight → build → publish) run only after this explicit action and can
stop on any failed gate.

## 3. Physical acceptance and evidence Issue

Before starting the official release:

1. Verify successful routine main-push CI, manually run CI on `main` (all five jobs must actually pass; no skips), and Windows QA on the *same final SHA of
   `main`*. PR-head QA artifacts do **not** qualify.
2. Use **Issues → New issue → Release bench acceptance / Стендовая приёмка**
   and record a real, isolated physical hardware test (not simulated CI).
3. Fill the required fields, **one per line**, with no Markdown prefix:

```text
Source SHA: <exact 40-character HEAD SHA of main>
Bench result: PASS
Hardware: <actual model and firmware>
Transport: <RTU/ASCII/TCP parameters>
Test summary: <observed results>
```

4. Supply links or attachments for logs, wiring/serial settings, expected
   versus actual responses, Modbus exception / timeout behaviour, version,
   tester, date, and safety checks. Record `FAIL` or `PENDING` if appropriate.
   The Release gate will reject either.
5. Release preflight fetches the **actual issue** through GitHub API. It checks
   that it belongs to this repository, is not a PR, contains `PASS` and the
   exact source SHA, and has nonempty hardware/transport/test details.
   The fact that an Issue reports PASS is a maintainer attestation, **not** an
   automated proof that hardware passed.

## 4. Manual release

From **main**, run *Release (manual, verified)* with:

- `tag = vX.Y.Z` (must match npm, Cargo, Tauri, lockfiles and not exist yet).
- `qa_run_id` = a successful Windows QA run on the exact final main SHA.
- `bench_passed = true` after physical acceptance only.
- `bench_evidence_url` = repository GitHub Issue URL, without fragments.

The preflight checks current main and checkout SHA, active main/tag Rulesets,
successful main-push CI plus all five actually successful jobs in a manual full-platform run on the exact SHA, Windows QA completion and artifact, actual
bench Issue fields and release/tag nonexistence. Windows packaging repeats
the tests and produces EXE, MSI, SHA256SUMS and BUILD-INFO. The publish job
rechecks gates, verifies checksums and creates a **new** versioned GitHub
Release. It cannot overwrite a published tag/release without changing policy.

## 5. Operational limitations

Only an administrator can edit the live Ruleset. The JSON template in Git
alone will not change it. Until the live rule is changed from one approval to
zero, your own PRs may be blocked; until it matches this policy, Release
preflight deliberately refuses publication.

A manual PASS and issue log do not constitute IEC 62443 certification or
guarantee that every industrial device behaves identically. Validate with
the relevant hardware, gateways, firmware and operational safety procedures.

## Live settings verification — 2026-11-21

**Important distinction:** this page describes the **proposed PR #10 policy**. As read from GitHub on 2026-11-21, the *active* main Ruleset `27728708` still has `required_approving_review_count = 1` and `require_last_push_approval = true`; the `v*` tag Ruleset `27728718` is active. Do not report solo-maintainer merging as enabled until GitHub Settings is edited and verified. No hardware PASS evidence was established in this audit. See [dated facts](AUDIT_2026-11-21.md) and [documentation index](INDEX.md).


## Windows-first transition (PR #23)
Routine PR/push CI runs the Windows Rust backend, platform-independent frontend/governance and separate Windows QA. Native Linux/macOS jobs are skipped at job level; GitHub retains their required status contexts for compatibility. This does not establish Linux/macOS compatibility. On the final main SHA use Actions → CI → Run workflow to execute all three platforms. The release gate verifies actual successful job conclusions, not only the workflow result or skipped status. All existing live Ruleset contexts and immutable tags remain unchanged. Linux/macOS installer build workflow is deferred to final development. The historical audit above is not the current state: live Ruleset 27728708 verified at PR #22 review with zero mandatory reviewers, resolved threads and strict checks.
