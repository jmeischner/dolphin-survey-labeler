import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const [, , version] = process.argv;

if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("Usage: node scripts/bump-version.mjs X.Y.Z[-pre]");
  process.exit(1);
}

const repoRoot = path.resolve(process.cwd());

const gitStatus = execSync("git status --porcelain", { encoding: "utf8" }).trim();
if (gitStatus.length > 0) {
  console.error("Working tree is not clean. Commit or stash changes first.");
  process.exit(1);
}

const updateJsonVersion = (filePath) => {
  const raw = fs.readFileSync(filePath, "utf8");
  const data = JSON.parse(raw);
  data.version = version;
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n");
};

const updateTauriVersion = (filePath) => {
  updateJsonVersion(filePath);
};

const updateCargoTomlVersion = (filePath) => {
  const raw = fs.readFileSync(filePath, "utf8");
  const next = raw.replace(
    /(^\s*version\s*=\s*")([^"]+)("\s*)$/m,
    `$1${version}$3`
  );
  if (next === raw) {
    console.error(`No version field updated in ${filePath}`);
    process.exit(1);
  }
  fs.writeFileSync(filePath, next);
};

updateJsonVersion(path.join(repoRoot, "package.json"));
updateTauriVersion(path.join(repoRoot, "src-tauri", "tauri.conf.json"));
updateCargoTomlVersion(path.join(repoRoot, "src-tauri", "Cargo.toml"));

execSync("cargo generate-lockfile", {
  stdio: "inherit",
  cwd: path.join(repoRoot, "src-tauri"),
});

console.log(`Updated versions to ${version}`);
