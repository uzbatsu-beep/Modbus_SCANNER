import test from "node:test";
import assert from "node:assert/strict";
import { validateEvidence, REQUIRED_CHECKS } from "./verify-release-gates.mjs";

const SHA = "a".repeat(40);
const branch = {
  name: "main",
  target: "branch", enforcement: "active",
  conditions: { ref_name: { include: ["~DEFAULT_BRANCH"], exclude: [] } },
  rules: [
    { type: "deletion" }, { type: "non_fast_forward" },
    { type: "pull_request", parameters: {
      required_approving_review_count: 1,
      dismiss_stale_reviews_on_push: true,
      require_last_push_approval: true,
      required_review_thread_resolution: true,
    } },
    { type: "required_status_checks", parameters: {
      strict_required_status_checks_policy: true,
      required_status_checks: REQUIRED_CHECKS.map((context) => ({ context })),
    } },
  ],
};
const tags = {
  name: "tags", target: "tag", enforcement: "active",
  conditions: { ref_name: { include: ["refs/tags/v*"], exclude: [] } },
  rules: [{ type: "deletion" }, { type: "update" }, { type: "non_fast_forward" }],
};

function fixture() {
  return {
    repo: "uzbatsu-beep/Modbus_SCANNER", tag: "v1.0.1", version: "1.0.1",
    sourceSha: SHA, checkoutSha: SHA, mainSha: SHA,
    benchPassed: true,
    benchUrl: "https://github.com/uzbatsu-beep/Modbus_SCANNER/issues/42",
    ciRuns: [{ head_sha: SHA, head_branch: "main", event: "push",
      status: "completed", conclusion: "success",
      path: ".github/workflows/ci.yml@refs/heads/main" }],
    qaRun: { id: 15, head_sha: SHA, head_branch: "main",
      status: "completed", conclusion: "success", event: "workflow_dispatch",
      path: ".github/workflows/windows-qa.yml@refs/heads/main" },
    qaJobs: [{ status: "completed", conclusion: "success" }],
    qaArtifacts: [{ name: "modbus-scanner-v1.0.1-windows-qa-33",
      expired: false, size_in_bytes: 12800000 }],
    rulesets: [branch, tags],
  };
}

test("correct release evidence passes every gate", () => {
  const f = fixture();
  assert.deepEqual(validateEvidence(f), {
    tag: "v1.0.1", sourceSha: SHA, qaRunId: 15, benchUrl: f.benchUrl,
  });
});
const cases = [
  ["refusing unapproved hardware", (f) => { f.benchPassed = false; }, /bench/],
  ["refusing missing evidence", (f) => { f.benchUrl = ""; }, /bench/],
  ["refusing external fake report", (f) => { f.benchUrl = "https://evil.example/issues/1"; }, /bench/],
  ["refusing wrong source checkout", (f) => { f.checkoutSha = "b".repeat(40); }, /identical/],
  ["refusing old main", (f) => { f.mainSha = "b".repeat(40); }, /identical/],
  ["refusing mismatched tag", (f) => { f.tag = "v1.0.2"; }, /version/],
  ["refusing release overwrite", (f) => { f.releaseAlreadyExists = true; }, /already exists/],
  ["refusing tag replacement", (f) => { f.tagAlreadyExists = true; }, /already exists/],
  ["refusing missing main CI", (f) => { f.ciRuns = []; }, /CI/],
  ["refusing PR-only CI", (f) => { f.ciRuns[0].event = "pull_request"; }, /CI/],
  ["refusing a stale QA commit", (f) => { f.qaRun.head_sha = "b".repeat(40); }, /QA/],
  ["refusing PR QA", (f) => { f.qaRun.event = "pull_request"; }, /QA/],
  ["refusing failed QA job", (f) => { f.qaJobs[0].conclusion = "failure"; }, /QA/],
  ["refusing expired QA artifact", (f) => { f.qaArtifacts[0].expired = true; }, /artifact/],
  ["refusing missing rulesets", (f) => { f.rulesets = []; }, /ruleset/],
  ["refusing inactive branch rules", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], enforcement: "disabled" }; }, /ruleset/],
  ["refusing no independent approval", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.map((r) =>
      r.type === "pull_request" ? { ...r, parameters: {
        ...r.parameters, required_approving_review_count: 0 } } : r) }; }, /approval/],
  ["refusing missing required check", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.map((r) =>
      r.type === "required_status_checks" ? { ...r, parameters: {
        ...r.parameters, required_status_checks: [] } } : r) }; }, /CI jobs/],
  ["refusing mutable release tags", (f) => { f.rulesets[1] = {
    ...f.rulesets[1], rules: [{ type: "deletion" }] }; }, /update/],
  ["refusing tag updates even if fast-forward", (f) => { f.rulesets[1] = {
    ...f.rulesets[1], rules: [{ type: "deletion" }, { type: "non_fast_forward" }] }; }, /update/],
];
for (const [name, mutate, expected] of cases) {
  test(name, () => {
    const value = fixture();
    mutate(value);
    assert.throws(() => validateEvidence(value), expected);
  });
}
