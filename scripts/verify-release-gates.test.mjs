import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { validateEvidence, REQUIRED_CHECKS, DEVELOPMENT_CHECKS } from "./verify-release-gates.mjs";

const SHA = "a".repeat(40);
const branch = {
  name: "main",
  target: "branch", enforcement: "active",
  conditions: { ref_name: { include: ["~DEFAULT_BRANCH"], exclude: [] } },
  rules: [
    { type: "deletion" }, { type: "non_fast_forward" },
    { type: "pull_request", parameters: {
      required_approving_review_count: 0,
      dismiss_stale_reviews_on_push: false,
      require_code_owner_review: false,
      require_last_push_approval: false,
      require_extra_approval_for_unattributed_changes: false,
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
    benchEvidence: { number: 42,
      html_url: "https://github.com/uzbatsu-beep/Modbus_SCANNER/issues/42",
      body: "Source SHA: " + SHA + "\nBench result: PASS\nHardware: test drive\nTransport: Modbus RTU\nTest summary: verified FC03 and timeout\n",
      pull_request: undefined },
    ciRuns: [{ head_sha: SHA, head_branch: "main", event: "push",
      status: "completed", conclusion: "success",
      path: ".github/workflows/ci.yml" }],
    fullCiRuns: [{ head_sha: SHA, head_branch: "main", event: "workflow_dispatch",
      status: "completed", conclusion: "success", path: ".github/workflows/ci.yml",
      jobs: REQUIRED_CHECKS.map((name) => ({ name, status: "completed", conclusion: "success" })) }],
    qaRun: { id: 15, head_sha: SHA, head_branch: "main",
      status: "completed", conclusion: "success", event: "workflow_dispatch",
      path: ".github/workflows/windows-qa.yml" },
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
  ["refusing missing Issue", (f) => { f.benchEvidence = undefined; }, /bench/],
  ["refusing a PR posing as bench Issue", (f) => { f.benchEvidence.pull_request = {}; }, /bench/],
  ["refusing wrong bench SHA", (f) => { f.benchEvidence.body = f.benchEvidence.body.replace(SHA, "b".repeat(40)); }, /bench/],
  ["refusing failed bench result", (f) => { f.benchEvidence.body = f.benchEvidence.body.replace("Bench result: PASS", "Bench result: FAIL"); }, /bench/],
  ["refusing an empty bench protocol", (f) => { f.benchEvidence.body = f.benchEvidence.body.replace("Hardware: test drive", "Hardware: "); }, /bench/],
  ["refusing bench placeholders", (f) => { f.benchEvidence.body = f.benchEvidence.body.replace("Hardware: test drive", "Hardware: REPLACE_WITH_DEVICE"); }, /bench/],
  ["refusing external fake report", (f) => { f.benchUrl = "https://evil.example/issues/1"; }, /bench/],
  ["refusing wrong source checkout", (f) => { f.checkoutSha = "b".repeat(40); }, /identical/],
  ["refusing old main", (f) => { f.mainSha = "b".repeat(40); }, /identical/],
  ["refusing mismatched tag", (f) => { f.tag = "v1.0.2"; }, /version/],
  ["refusing release overwrite", (f) => { f.releaseAlreadyExists = true; }, /already exists/],
  ["refusing tag replacement", (f) => { f.tagAlreadyExists = true; }, /already exists/],
  ["refusing missing main CI", (f) => { f.ciRuns = []; }, /CI/],
  ["refusing missing final cross-platform CI", (f) => { f.fullCiRuns = []; }, /full three-platform CI/],
  ["refusing skipped Linux test", (f) => { f.fullCiRuns[0].jobs[3].conclusion = "skipped"; }, /full three-platform CI/],
  ["refusing skipped macOS test", (f) => { f.fullCiRuns[0].jobs[4].conclusion = "skipped"; }, /full three-platform CI/],
  ["refusing missing final platform job", (f) => { f.fullCiRuns[0].jobs.pop(); }, /full three-platform CI/],
  ["refusing failed final platform job", (f) => { f.fullCiRuns[0].jobs[4].conclusion = "failure"; }, /full three-platform CI/],
  ["refusing final CI on old SHA", (f) => { f.fullCiRuns[0].head_sha = "b".repeat(40); }, /full three-platform CI/],
  ["refusing final CI on another branch", (f) => { f.fullCiRuns[0].head_branch = "feature"; }, /full three-platform CI/],
  ["refusing routine run as final CI", (f) => { f.fullCiRuns[0].event = "push"; }, /full three-platform CI/],
  ["refusing wrong final workflow", (f) => { f.fullCiRuns[0].path = ".github/workflows/ci.yml.untrusted"; }, /full three-platform CI/],
  ["refusing PR-only CI", (f) => { f.ciRuns[0].event = "pull_request"; }, /CI/],
  ["refusing similar but wrong CI workflow", (f) => {
    f.ciRuns[0].path = ".github/workflows/ci.yml.insecure"; }, /CI/],
  ["refusing similar but wrong QA workflow", (f) => {
    f.qaRun.path = ".github/workflows/windows-qa.yml.insecure"; }, /QA/],
  ["refusing a stale QA commit", (f) => { f.qaRun.head_sha = "b".repeat(40); }, /QA/],
  ["refusing PR QA", (f) => { f.qaRun.event = "pull_request"; }, /QA/],
  ["refusing failed QA job", (f) => { f.qaJobs[0].conclusion = "failure"; }, /QA/],
  ["refusing expired QA artifact", (f) => { f.qaArtifacts[0].expired = true; }, /artifact/],
  ["refusing missing rulesets", (f) => { f.rulesets = []; }, /ruleset/],
  ["refusing inactive branch rules", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], enforcement: "disabled" }; }, /ruleset/],
  ["refusing a mandatory second-person review", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.map((r) =>
      r.type === "pull_request" ? { ...r, parameters: {
        ...r.parameters, required_approving_review_count: 1 } } : r) }; }, /approval/],
  ["refusing last-push approval requirement", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.map((r) =>
      r.type === "pull_request" ? { ...r, parameters: {
        ...r.parameters, require_last_push_approval: true } } : r) }; }, /approval/],
  ["refusing codeowner approval requirement", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.map((r) =>
      r.type === "pull_request" ? { ...r, parameters: {
        ...r.parameters, require_code_owner_review: true } } : r) }; }, /approval/],
  ["refusing no pull-request rule", (f) => { f.rulesets[0] = {
    ...f.rulesets[0], rules: f.rulesets[0].rules.filter((r) =>
      r.type !== "pull_request") }; }, /approval/],
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


test("repository main Ruleset template requires PR but no second-person reviewer", () => {
  const rule = JSON.parse(readFileSync(new URL("../.github/rulesets/main.json", import.meta.url), "utf8"));
  assert.equal(rule.target, "branch");
  assert.equal(rule.enforcement, "active");
  assert.deepEqual(rule.bypass_actors, []);
  const pr = rule.rules.find((r) => r.type === "pull_request");
  assert.equal(pr.parameters.required_approving_review_count, 0);
  assert.equal(pr.parameters.require_last_push_approval, false);
  assert.equal(pr.parameters.require_code_owner_review, false);
  assert.equal(pr.parameters.required_review_thread_resolution, true);
  const checks = rule.rules.find((r) => r.type === "required_status_checks");
  assert.equal(checks.parameters.strict_required_status_checks_policy, true);
  assert.deepEqual(checks.parameters.required_status_checks.map((k) => k.context).sort(), [...DEVELOPMENT_CHECKS].sort());
  for (const type of ["deletion", "non_fast_forward"]) {
    assert.ok(rule.rules.some((r) => r.type === type));
  }
});

test("repository tag Ruleset template protects tag updates and deletion", () => {
  const rule = JSON.parse(readFileSync(new URL("../.github/rulesets/release-tags.json", import.meta.url), "utf8"));
  assert.equal(rule.target, "tag");
  assert.equal(rule.enforcement, "active");
  assert.deepEqual(rule.bypass_actors, []);
  for (const type of ["deletion", "update", "non_fast_forward"]) {
    assert.ok(rule.rules.some((r) => r.type === type));
  }
});

test("physical bench Issue template records every attestation field", () => {
  const template = readFileSync(new URL("../.github/ISSUE_TEMPLATE/release_bench_acceptance.md", import.meta.url), "utf8");
  for (const marker of ["Source SHA:", "Bench result: PENDING", "Hardware:", "Transport:", "Test summary:"]) {
    assert.ok(template.includes(marker), marker);
  }
});


test("Windows-first workflow preserves required names and defers native Linux/macOS jobs", () => {
  const workflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
  assert.match(workflow, /^  workflow_dispatch:/m);
  for (const name of REQUIRED_CHECKS) assert.ok(workflow.includes("name: " + name), name);
  for (const [job, runner] of [["backend-linux", "ubuntu-latest"], ["backend-macos", "macos-latest"]]) {
    const block = workflow.split("  " + job + ":\n")[1].split(/\n  [a-z][\w-]*:/)[0];
    assert.ok(block.includes("if: github.event_name == 'workflow_dispatch'"));
    assert.ok(block.includes("runs-on: " + runner));
    assert.ok(block.includes("cargo test --lib"));
  }
  const windows = workflow.split("  backend:\n")[1].split("\n  backend-linux:")[0];
  assert.ok(windows.includes("runs-on: windows-latest"));
  assert.ok(!windows.includes("github.event_name == 'workflow_dispatch'"));
});


test("final Ruleset template restores exactly five required contexts without relaxing protection", () => {
  const dev = JSON.parse(readFileSync(new URL("../.github/rulesets/main.json", import.meta.url), "utf8"));
  const final = JSON.parse(readFileSync(new URL("../.github/rulesets/main-final.json", import.meta.url), "utf8"));
  const contexts = final.rules.find((r) => r.type === "required_status_checks").parameters.required_status_checks;
  assert.deepEqual(contexts.map((k) => k.context).sort(), [...REQUIRED_CHECKS].sort());
  const normalized = structuredClone(final);
  normalized.name = dev.name;
  normalized.rules.find((r) => r.type === "required_status_checks").parameters.required_status_checks =
    dev.rules.find((r) => r.type === "required_status_checks").parameters.required_status_checks;
  assert.deepEqual(normalized, dev);
});
test("official release refuses development-only Ruleset even with successful final CI", () => {
  const f = fixture();
  f.rulesets = [JSON.parse(readFileSync(new URL("../.github/rulesets/main.json", import.meta.url), "utf8")), tags];
  assert.throws(() => validateEvidence(f), /restore all 5 strict CI jobs/);
});
