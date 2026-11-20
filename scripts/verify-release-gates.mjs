// Release preflight for PR #9. Fail closed: no interactive inputs, no fallback
// to merely green PR CI, and no automatic tag-triggered publication.
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

export const REQUIRED_CHECKS = [
  "Frontend (typecheck + build)",
  "Governance (release gate tests)",
  "Backend (cargo check + tests) (windows-latest)",
  "Backend (cargo check + tests) (ubuntu-latest)",
  "Backend (cargo check + tests) (macos-latest)",
];

function requireGate(condition, message) {
  if (!condition) throw new Error("RELEASE BLOCKED: " + message);
}

function includesRef(ruleset, ref) {
  const names = ruleset.conditions?.ref_name?.include ?? [];
  const excludes = ruleset.conditions?.ref_name?.exclude ?? [];
  return !excludes.includes(ref) &&
    names.some((name) => name === ref || (ref === "refs/heads/main" && name === "~DEFAULT_BRANCH") ||
      (ref.startsWith("refs/tags/v") && name === "refs/tags/v*"));
}

function isWorkflowPath(actual, expected) {
  // GitHub Actions REST run.path is usually ".github/workflows/name.yml".
  // Some API variants append "@refs/heads/main"; accept both, but not
  // similarly prefixed file names such as "ci.yml.untrusted".
  return actual === expected || String(actual ?? "").startsWith(expected + "@");
}

function activeRule(rulesets, target, ref, type) {
  return rulesets.filter((r) => r.target === target && r.enforcement === "active" &&
    includesRef(r, ref)).flatMap((r) => r.rules ?? []).find((r) => r.type === type);
}

export function validateEvidence({
  repo, tag, version, sourceSha, checkoutSha, mainSha, benchPassed, benchUrl,
  ciRuns, qaRun, qaJobs, qaArtifacts, rulesets, benchEvidence, tagAlreadyExists = false,
  releaseAlreadyExists = false,
}) {
  requireGate(repo === "uzbatsu-beep/Modbus_SCANNER", "unexpected repository");
  requireGate(/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag), "invalid release tag");
  requireGate(tag === "v" + version, "tag and package/tauri/Cargo version differ");
  requireGate(/^[0-9a-f]{40}$/.test(sourceSha), "invalid source SHA");
  requireGate(sourceSha === checkoutSha && sourceSha === mainSha,
    "checkout, workflow commit and current main must be identical");
  requireGate(benchPassed === true, "isolated bench acceptance was not approved");
  const benchMatch = typeof benchUrl === "string" &&
    benchUrl.match(/^https:\/\/github\.com\/uzbatsu-beep\/Modbus_SCANNER\/issues\/([1-9]\d*)$/);
  requireGate(benchMatch, "bench evidence must link to a repository GitHub Issue");
  requireGate(benchEvidence && !benchEvidence.pull_request &&
    benchEvidence.html_url === benchUrl &&
    benchEvidence.number === Number(benchMatch[1]),
    "bench evidence Issue was not found or is not a standalone issue");
  const body = String(benchEvidence.body ?? "");
  requireGate(new RegExp("^Source SHA:\\s*" + sourceSha + "\\s*$", "im").test(body) &&
    /^Bench result:\s*PASS\s*$/im.test(body),
    "bench Issue must contain exact Source SHA and Bench result: PASS");
  for (const label of ["Hardware", "Transport", "Test summary"]) {
    requireGate(new RegExp("^" + label + ":\\s*\\S.*$", "im").test(body),
      "bench Issue is missing required field: " + label);
  }
  requireGate(!tagAlreadyExists && !releaseAlreadyExists,
    "release tag or release already exists: do not overwrite");

  const pr = activeRule(rulesets, "branch", "refs/heads/main", "pull_request");
  // Solo maintainer: PRs and strict CI remain mandatory, but no other human
  // can be required to approve a maintainer-authored PR or release.
  requireGate(pr && pr.parameters?.required_approving_review_count === 0 &&
    pr.parameters?.require_last_push_approval === false &&
    pr.parameters?.require_code_owner_review !== true &&
    pr.parameters?.require_extra_approval_for_unattributed_changes !== true &&
    pr.parameters?.required_review_thread_resolution === true,
    "main ruleset must require PR without mandatory second-person approval");
  const status = activeRule(rulesets, "branch", "refs/heads/main", "required_status_checks");
  const contexts = (status?.parameters?.required_status_checks ?? []).map((x) => x.context);
  requireGate(status?.parameters?.strict_required_status_checks_policy === true &&
    REQUIRED_CHECKS.every((check) => contexts.includes(check)),
    "main ruleset must require all 5 strict CI jobs");
  for (const ruleType of ["deletion", "non_fast_forward"]) {
    requireGate(activeRule(rulesets, "branch", "refs/heads/main", ruleType),
      "main ruleset must block " + ruleType);
  }
  for (const ruleType of ["deletion", "update", "non_fast_forward"]) {
    requireGate(activeRule(rulesets, "tag", "refs/tags/" + tag, ruleType),
      "v* tag ruleset must block " + ruleType);
  }

  requireGate(Array.isArray(ciRuns) && ciRuns.some((r) =>
    r.head_sha === sourceSha && r.head_branch === "main" &&
    r.event === "push" && r.status === "completed" && r.conclusion === "success" &&
    isWorkflowPath(r.path, ".github/workflows/ci.yml")),
    "no successful main-push CI for EXACT release commit");
  requireGate(qaRun && qaRun.head_sha === sourceSha && qaRun.head_branch === "main" &&
    ["push", "workflow_dispatch"].includes(qaRun.event) &&
    qaRun.status === "completed" && qaRun.conclusion === "success" &&
    isWorkflowPath(qaRun.path, ".github/workflows/windows-qa.yml"),
    "Windows QA must pass on the EXACT main commit (not a PR head)");
  requireGate(Array.isArray(qaJobs) && qaJobs.length > 0 &&
    qaJobs.every((j) => j.status === "completed" && j.conclusion === "success"),
    "Windows QA must finish all jobs successfully");
  requireGate(Array.isArray(qaArtifacts) && qaArtifacts.some((a) =>
    a.expired === false && Number(a.size_in_bytes) > 0 &&
    String(a.name ?? "").startsWith("modbus-scanner-v" + version + "-windows-qa-")),
    "Windows QA installer artifact is missing or expired");
  return { tag, sourceSha, qaRunId: qaRun.id, benchUrl };
}

async function githubApi(repo, relative, { allowNotFound = false } = {}) {
  const token = process.env.GITHUB_TOKEN;
  requireGate(!!token, "GITHUB_TOKEN not available");
  const response = await fetch("https://api.github.com/repos/" + repo + "/" + relative, {
    headers: {
      Authorization: "Bearer " + token,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "modbus-scanner-release-gate",
    },
  });
  if (allowNotFound && response.status === 404) return null;
  if (!response.ok) throw new Error("GitHub API " + relative + ": HTTP " + response.status +
    " " + (await response.text()).slice(0, 250));
  return response.json();
}

async function main() {
  const repo = process.env.GITHUB_REPOSITORY;
  const tag = process.env.RELEASE_TAG ?? "";
  const qaRunId = process.env.QA_RUN_ID ?? "";
  const sourceSha = process.env.GITHUB_SHA ?? "";
  const benchUrl = process.env.BENCH_EVIDENCE_URL ?? "";
  requireGate(process.env.GITHUB_REF === "refs/heads/main", "run release only on main");
  requireGate(/^\d+$/.test(qaRunId) && Number(qaRunId) > 0, "invalid QA run ID");
  const version = JSON.parse(readFileSync("package.json", "utf8")).version;
  const checkoutSha = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const benchMatch = benchUrl.match(/^https:\/\/github\.com\/uzbatsu-beep\/Modbus_SCANNER\/issues\/([1-9]\d*)$/);
  requireGate(benchMatch, "bench evidence URL must be a repository GitHub Issue");
  const [mainRef, ci, qaRun, qa, qaArtifacts, list, tagRef, release, benchEvidence] = await Promise.all([
    githubApi(repo, "git/ref/heads/main"),
    githubApi(repo, "actions/workflows/ci.yml/runs?head_sha=" + sourceSha +
      "&branch=main&event=push&status=success&per_page=100"),
    githubApi(repo, "actions/runs/" + qaRunId),
    githubApi(repo, "actions/runs/" + qaRunId + "/jobs?per_page=100"),
    githubApi(repo, "actions/runs/" + qaRunId + "/artifacts?per_page=100"),
    githubApi(repo, "rulesets?includes_parents=true&per_page=100"),
    githubApi(repo, "git/ref/tags/" + encodeURIComponent(tag), { allowNotFound: true }),
    githubApi(repo, "releases/tags/" + encodeURIComponent(tag), { allowNotFound: true }),
    githubApi(repo, "issues/" + benchMatch[1]),
  ]);
  const active = (list ?? []).filter((r) => r.enforcement === "active");
  const rulesets = await Promise.all(active.map((r) => githubApi(repo, "rulesets/" + r.id)));
  const result = validateEvidence({
    repo, tag, version, sourceSha, checkoutSha, mainSha: mainRef.object.sha,
    benchPassed: process.env.BENCH_PASSED === "true", benchUrl,
    ciRuns: ci.workflow_runs, qaRun, qaJobs: qa.jobs, qaArtifacts: qaArtifacts.artifacts,
    rulesets, benchEvidence, tagAlreadyExists: !!tagRef, releaseAlreadyExists: !!release,
  });
  console.log("Release preflight PASS: all 5 CI checks, exact SHA QA, active rulesets, " +
    "same-SHA PASS bench Issue and immutable tag requirements verified.");
  console.log(JSON.stringify(result, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
