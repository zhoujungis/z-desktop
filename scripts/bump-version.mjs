#!/usr/bin/env node
/**
 * 同步提升项目版本号。
 *
 *   node scripts/bump-version.mjs --bump patch
 *   node scripts/bump-version.mjs --bump minor
 *   node scripts/bump-version.mjs --bump major
 *   node scripts/bump-version.mjs --set 1.2.3
 *   node scripts/bump-version.mjs --bump patch --dry-run
 *
 * 会同步更新：
 *   package.json
 *   package-lock.json
 *   src-tauri/tauri.conf.json
 *   src-tauri/Cargo.toml
 *   src-tauri/Cargo.lock
 *
 * stdout 只输出新版本号，方便 CI 捕获。
 */
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");

function argValue(name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function writeJson(file, data) {
  if (!dryRun) fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}

function bumpVersion(current, bump) {
  const match = current.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/);
  if (!match) throw new Error(`不支持当前版本号格式: ${current}`);
  let major = Number(match[1]);
  let minor = Number(match[2]);
  let patch = Number(match[3]);
  const prerelease = match[4];

  switch (bump) {
    case "patch":
      // 预发布版本执行 patch 时先转为正式版
      if (!prerelease) patch += 1;
      break;
    case "minor":
      minor += 1;
      patch = 0;
      break;
    case "major":
      major += 1;
      minor = 0;
      patch = 0;
      break;
    default:
      throw new Error(`未知的 bump 类型: ${bump}（可选 patch / minor / major）`);
  }
  return `${major}.${minor}.${patch}`;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const pkgPath = path.join(root, "package.json");
const pkg = readJson(pkgPath);
const current = pkg.version;
const explicit = argValue("--set");
const bump = argValue("--bump") ?? "patch";
const version = explicit ?? bumpVersion(current, bump);

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`无效版本号: ${version}`);
}

const changes = [];

pkg.version = version;
writeJson(pkgPath, pkg);
changes.push("package.json");

const lockPath = path.join(root, "package-lock.json");
if (fs.existsSync(lockPath)) {
  const lock = readJson(lockPath);
  lock.version = version;
  if (lock.packages && lock.packages[""]) {
    lock.packages[""].version = version;
  }
  writeJson(lockPath, lock);
  changes.push("package-lock.json");
}

const tauriPath = path.join(root, "src-tauri", "tauri.conf.json");
if (fs.existsSync(tauriPath)) {
  // 只替换顶层 version，避免 JSON.stringify 重排数组造成无意义 diff
  const tauri = fs.readFileSync(tauriPath, "utf8");
  const nextTauri = tauri.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
  if (nextTauri === tauri) {
    throw new Error("tauri.conf.json 中未找到 version 字段");
  }
  if (!dryRun) fs.writeFileSync(tauriPath, nextTauri);
  changes.push("src-tauri/tauri.conf.json");
}

const cargoPath = path.join(root, "src-tauri", "Cargo.toml");
const cargo = fs.readFileSync(cargoPath, "utf8");
const cargoName = cargo.match(/^name\s*=\s*"([^"]+)"/m)?.[1] ?? "z-desktop";
const nextCargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
if (nextCargo === cargo) {
  throw new Error("Cargo.toml 中未找到 version 字段");
}
if (!dryRun) fs.writeFileSync(cargoPath, nextCargo);
changes.push("src-tauri/Cargo.toml");

const cargoLockPath = path.join(root, "src-tauri", "Cargo.lock");
if (fs.existsSync(cargoLockPath)) {
  const cargoLock = fs.readFileSync(cargoLockPath, "utf8");
  const pattern = new RegExp(
    `(\\[\\[package\\]\\]\\s*\\nname = "${escapeRegExp(cargoName)}"\\s*\\nversion = ")[^"]+(")`,
  );
  if (!pattern.test(cargoLock)) {
    throw new Error(`Cargo.lock 中未找到 ${cargoName} 的版本记录`);
  }
  if (!dryRun) {
    fs.writeFileSync(cargoLockPath, cargoLock.replace(pattern, `$1${version}$2`));
  }
  changes.push("src-tauri/Cargo.lock");
}

if (dryRun) {
  process.stderr.write(
    `[dry-run] ${current} -> ${version} (${changes.join(", ")})\n`,
  );
}
process.stdout.write(`${version}\n`);
