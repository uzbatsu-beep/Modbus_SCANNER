/**
 * Fail packaging before compilation when version manifests disagree.
 * Release tags are also checked, preventing accidental wrong-version uploads.
 * Runs with Node.js >=18, without external dependencies.
 */
import { readFileSync } from "node:fs";

const load = (path) => readFileSync(new URL("../" + path, import.meta.url), "utf8");
const json = (path) => JSON.parse(load(path));
const version = json("package.json").version;
const lock = json("package-lock.json");
const tauri = json("src-tauri/tauri.conf.json");
const cargo = load("src-tauri/Cargo.toml");
const cargoLock = load("src-tauri/Cargo.lock");

function requireMatch(label, actual) {
  if (actual !== version) {
    throw new Error(`Version mismatch: ${label}=${actual ?? "missing"}, package.json=${version}`);
  }
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`Invalid release candidate version: ${version}`);
}

const cargoPackage = cargo.match(/^\[package\]([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1];
const cargoVersion = cargoPackage?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const lockVersion = cargoLock.match(/\[\[package\]\]\s*\nname\s*=\s*"modbus-scanner"\s*\nversion\s*=\s*"([^"]+)"/m)?.[1];

requireMatch("package-lock.json", lock.version);
requireMatch("package-lock.json root package", lock.packages?.[""]?.version);
requireMatch("tauri.conf.json", tauri.version);
requireMatch("Cargo.toml", cargoVersion);
requireMatch("Cargo.lock modbus-scanner", lockVersion);

if (process.env.GITHUB_REF_TYPE === "tag") {
  requireMatch("Git tag", process.env.GITHUB_REF_NAME?.replace(/^v/, ""));
}

console.log(`Release version verified: ${version} (npm, lockfiles, Cargo, Tauri, Git tag if present)`);
