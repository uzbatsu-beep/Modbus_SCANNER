#!/usr/bin/env node
/**
 * Offline Markdown relative-link checker. No external URL probing or dependencies.
 * Scope: links of the form [text](relative/path) and ![alt](relative/image).
 * Intentional limitation: fragments (#anchor) and external URLs are not validated.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SKIP_DIRS = new Set(['.git', 'node_modules', 'dist', 'target', '.vite']);

export function markdownFiles(root) {
  const files = [];
  function visit(dir) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) visit(path.join(dir, entry.name));
      } else if (entry.isFile() && entry.name.toLowerCase().endsWith('.md')) {
        files.push(path.join(dir, entry.name));
      }
    }
  }
  visit(root);
  return files.sort();
}

export function brokenLocalLinks(root) {
  const issues = [];
  for (const file of markdownFiles(root)) {
    // Code-fence examples must not count as navigation.
    const source = readFileSync(file, 'utf8').replace(/^(\s*)(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\s*\2\s*$/gm, '');
    // Deliberately supports ordinary Markdown destinations, not full CommonMark grammar.
    const re = /!?\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|([^\s)]+))/g;
    for (const match of source.matchAll(re)) {
      let destination = match[1] ?? match[2] ?? '';
      if (/^(?:[a-z][a-z0-9+.-]*:|\/\/|#)/i.test(destination)) continue;
      destination = destination.split(/[?#]/, 1)[0];
      if (!destination) continue;
      try { destination = decodeURIComponent(destination); }
      catch { issues.push({file: path.relative(root,file), target: destination, reason: 'invalid URL encoding'}); continue; }
      const target = path.resolve(path.dirname(file), destination);
      const inRoot = target === root || target.startsWith(root + path.sep);
      if (!inRoot || !existsSync(target)) {
        issues.push({file: path.relative(root,file), target: destination,
          reason: inRoot ? 'missing file' : 'outside repository'});
      }
    }
  }
  return issues;
}

export function checkDocs(root = DEFAULT_ROOT) {
  if (!existsSync(root) || !statSync(root).isDirectory()) throw new Error('repo root missing');
  const files = markdownFiles(root);
  const issues = brokenLocalLinks(root);
  return { count: files.length, issues };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const {count, issues} = checkDocs();
  for (const issue of issues) console.error(`${issue.file}: ${issue.target} — ${issue.reason}`);
  console.log(`Markdown relative-link check: ${count} files, ${issues.length} broken link(s)`);
  if (issues.length) process.exitCode = 1;
}
