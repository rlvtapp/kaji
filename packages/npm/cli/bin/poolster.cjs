#!/usr/bin/env node
'use strict';

const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');
const { jsConfig, runJsConfig } = require('./config.cjs');

function platformPackage(platform = process.platform, arch = process.arch, report = process.report) {
  const key = `${platform}-${arch}`;
  if (key === 'linux-x64') {
    if (!report?.getReport().header.glibcVersionRuntime) {
      throw new Error('Poolster Linux binaries currently require glibc. Alpine/musl is not supported yet.');
    }
    return '@relevate/poolster-cli-linux-x64-gnu';
  }
  if (key === 'darwin-arm64' || key === 'darwin-x64') return `@relevate/poolster-cli-${key}`;
  if (key === 'win32-x64') return '@relevate/poolster-cli-win32-x64-msvc';
  throw new Error(`Poolster does not yet provide an npm binary for ${key}. Build crates/cli and openapi/ from source.`);
}

function resolveBinary() {
  if (process.env.POOLSTER_BINARY) return path.resolve(process.env.POOLSTER_BINARY);
  const packageName = platformPackage();
  let manifest;
  try {
    manifest = require.resolve(`${packageName}/package.json`);
  } catch {
    throw new Error(`Missing native package ${packageName}. Reinstall poolster with optional dependencies enabled (do not use --omit=optional).`);
  }
  const nativePackage = JSON.parse(fs.readFileSync(manifest, 'utf8'));
  const launcherPackage = require('../package.json');
  if (nativePackage.version !== launcherPackage.version) {
    throw new Error(`Native package version ${nativePackage.version} does not match launcher ${launcherPackage.version}. Reinstall poolster.`);
  }
  const executable = path.join(path.dirname(manifest), process.platform === 'win32' ? 'poolster.exe' : 'poolster');
  if (!fs.existsSync(executable)) throw new Error(`The native executable is missing: ${executable}. Reinstall poolster.`);
  return executable;
}

function main() {
  let configFile;
  try { configFile = jsConfig(process.argv.slice(2)); }
  catch (error) { console.error(`poolster: ${error.message}`); process.exitCode = 1; return; }
  if (configFile) {
    runJsConfig(process.argv.slice(2), configFile).then((code) => { process.exitCode = code; }).catch((error) => {
      console.error(`poolster: ${error.message}`);
      process.exitCode = 1;
    });
    return;
  }
  let executable;
  try { executable = resolveBinary(); }
  catch (error) { console.error(`poolster: ${error.message}`); process.exitCode = 1; return; }
  const child = spawn(executable, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
  let failed = false;
  const forward = (signal) => child.kill(signal);
  process.on('SIGINT', forward);
  process.on('SIGTERM', forward);
  child.on('error', (error) => {
    failed = true;
    console.error(`poolster: could not start ${executable}: ${error.message}`);
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
