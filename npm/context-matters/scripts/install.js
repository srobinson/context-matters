#!/usr/bin/env node

"use strict";

const { execSync } = require("child_process");
const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");

const REPO = "srobinson/context-matters";
const BIN_NAME = "cm";

const PLATFORM_MAP = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
};

function getPlatformKey() {
  const platform = process.platform;
  const arch = process.arch;
  return `${platform}-${arch}`;
}

function getTarget() {
  const key = getPlatformKey();
  const target = PLATFORM_MAP[key];
  if (!target) {
    console.error(
      `Unsupported platform: ${key}\n` +
        `Supported: ${Object.keys(PLATFORM_MAP).join(", ")}`,
    );
    process.exit(1);
  }
  return target;
}

function getVersion() {
  const pkg = JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8"),
  );
  return pkg.version;
}

function fetch(url) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith("https") ? https : http;
    mod
      .get(
        url,
        { headers: { "User-Agent": "context-matters-installer" } },
        (res) => {
          if (
            res.statusCode >= 300 &&
            res.statusCode < 400 &&
            res.headers.location
          ) {
            return fetch(res.headers.location).then(resolve, reject);
          }
          if (res.statusCode !== 200) {
            return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
          }
          const chunks = [];
          res.on("data", (chunk) => chunks.push(chunk));
          res.on("end", () => resolve(Buffer.concat(chunks)));
          res.on("error", reject);
        },
      )
      .on("error", reject);
  });
}

function computeSha256(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

async function verifySha256(tarball, artifact, version) {
  const sumsUrl = `https://github.com/${REPO}/releases/download/v${version}/sha256sums.txt`;
  try {
    const sumsData = await fetch(sumsUrl);
    const sumsText = sumsData.toString("utf8");
    const expectedHash = sumsText
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .map((line) => {
        const parts = line.split(/\s+/);
        return { hash: parts[0], file: parts[parts.length - 1] };
      })
      .find((entry) => entry.file === artifact);

    if (!expectedHash) {
      console.warn(
        `Warning: ${artifact} not found in sha256sums.txt, skipping verification`,
      );
      return true;
    }

    const actualHash = computeSha256(tarball);
    if (actualHash !== expectedHash.hash) {
      console.error(
        `SHA-256 mismatch for ${artifact}:\n` +
          `  expected: ${expectedHash.hash}\n` +
          `  actual:   ${actualHash}`,
      );
      return false;
    }

    console.log(`SHA-256 verified: ${actualHash.slice(0, 16)}...`);
    return true;
  } catch (err) {
    console.warn(
      `Warning: could not fetch sha256sums.txt (${err.message}), skipping verification`,
    );
    return true;
  }
}

async function install() {
  const target = getTarget();
  const version = getVersion();
  const artifact = `cm-${target}.tar.gz`;
  const url = `https://github.com/${REPO}/releases/download/v${version}/${artifact}`;

  const binDir = path.join(__dirname, "..", "bin");
  const binPath = path.join(binDir, BIN_NAME);

  // Skip if native binary already exists (e.g. CI caching)
  try {
    const stat = fs.statSync(binPath);
    if (stat.size > 10000) return;
  } catch {}

  console.log(`Downloading ${BIN_NAME} v${version} for ${target}...`);

  try {
    const tarball = await fetch(url);

    // SHA-256 verification
    const verified = await verifySha256(tarball, artifact, version);
    if (!verified) {
      console.error("Binary rejected due to SHA-256 mismatch.");
      process.exit(1);
    }

    const tmpTar = path.join(binDir, `${BIN_NAME}.tar.gz`);
    fs.writeFileSync(tmpTar, tarball);
    execSync(`tar xzf "${tmpTar}" -C "${binDir}"`, { stdio: "pipe" });
    fs.unlinkSync(tmpTar);

    // Ensure the binary is executable
    fs.chmodSync(binPath, 0o755);

    console.log(`Installed ${BIN_NAME} v${version} to ${binPath}`);
  } catch (err) {
    console.error(
      `Failed to download ${BIN_NAME} v${version} for ${target}:\n` +
        `  ${err.message}\n\n` +
        `You can install manually:\n` +
        `  cargo install --path crates/cm-cli`,
    );
    // Don't fail the install - the bin wrapper will show a helpful error
  }
}

install();
