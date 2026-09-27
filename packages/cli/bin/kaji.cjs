#!/usr/bin/env node
'use strict';

const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');

function platformPackage(platform = process.platform, arch = process.arch, report = process.report) {
  const key = `${platform}-${arch}`;
  if (key === 'linux-x64') {
    if (!report?.getReport().header.glibcVersionRuntime) {
      throw new Error('Kaji Linux binaries currently require glibc. Alpine/musl is not supported yet.');
    }
    return '@relevate/kaji-linux-x64-gnu';
  }
  if (key === 'darwin-arm64' || key === 'darwin-x64') return `@relevate/kaji-${key}`;
  if (key === 'win32-x64') return '@relevate/kaji-win32-x64-msvc';
  throw new Error(`Kaji does not yet provide an npm binary for ${key}. Build crates/kaji-cli and openapi/ from source.`);
}

function resolveBinary() {
  if (process.env.KAJI_BINARY) return path.resolve(process.env.KAJI_BINARY);
  const packageName = platformPackage();
  let manifest;
  try {
    manifest = require.resolve(`${packageName}/package.json`);
  } catch {
    throw new Error(`Missing native package ${packageName}. Reinstall @relevate/kaji with optional dependencies enabled (do not use --omit=optional).`);
  }
  const nativePackage = JSON.parse(fs.readFileSync(manifest, 'utf8'));
  const launcherPackage = require('../package.json');
  if (nativePackage.version !== launcherPackage.version) {
    throw new Error(`Native package version ${nativePackage.version} does not match launcher ${launcherPackage.version}. Reinstall @relevate/kaji.`);
  }
  const executable = path.join(path.dirname(manifest), process.platform === 'win32' ? 'kaji.exe' : 'kaji');
  if (!fs.existsSync(executable)) throw new Error(`The native executable is missing: ${executable}. Reinstall @relevate/kaji.`);
  return executable;
}

function main() {
  let executable;
  try { executable = resolveBinary(); }
  catch (error) { console.error(`kaji: ${error.message}`); process.exitCode = 1; return; }
  const child = spawn(executable, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
  let failed = false;
  const forward = (signal) => child.kill(signal);
  process.on('SIGINT', forward);
  process.on('SIGTERM', forward);
  child.on('error', (error) => {
    failed = true;
    console.error(`kaji: could not start ${executable}: ${error.message}`);
    process.exitCode = 1;
  });
  child.on('close', (code, signal) => {
    process.removeListener('SIGINT', forward);
    process.removeListener('SIGTERM', forward);
    if (signal) process.kill(process.pid, signal);
    else process.exitCode = failed ? 1 : (code ?? 1);
  });
}

module.exports = { main, platformPackage, resolveBinary };
if (require.main === module) main();
