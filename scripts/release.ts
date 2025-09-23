#!/usr/bin/env -S tsx

import { promises as fs } from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

const IGNORED_DIRECTORIES = new Set([
  '.git',
  'node_modules',
  'target',
  'dist',
  'build',
  '.pnpm-store'
]);

async function main(): Promise<void> {
  const version = process.argv[2];

  if (!version) {
    console.error('Usage: release <version>');
    process.exitCode = 1;
    return;
  }

  if (!isValidVersion(version)) {
    console.error(`Invalid version: ${version}`);
    process.exitCode = 1;
    return;
  }

  const manifests = await collectManifests(repoRoot);

  await Promise.all(manifests.packageJson.map(file => updatePackageJson(file, version)));
  await Promise.all(manifests.cargoToml.map(file => updateCargoToml(file, version)));

  await runCommand('pnpm', ['publish'], path.join(repoRoot, 'typescript'));
  await runCommand('cargo', ['publish'], path.join(repoRoot, 'rust'));
}

function isValidVersion(version: string): boolean {
  return /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version);
}

async function collectManifests(root: string): Promise<{ packageJson: string[]; cargoToml: string[] }> {
  const packageJson: string[] = [];
  const cargoToml: string[] = [];

  await walk(root, async filePath => {
    const base = path.basename(filePath);

    if (base === 'package.json') {
      packageJson.push(filePath);
    } else if (base === 'Cargo.toml') {
      cargoToml.push(filePath);
    }
  });

  return { packageJson, cargoToml };
}

async function walk(dir: string, onFile: (filePath: string) => Promise<void>): Promise<void> {
  const entries = await fs.readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    if (IGNORED_DIRECTORIES.has(entry.name)) {
      continue;
    }

    const fullPath = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      await walk(fullPath, onFile);
    } else if (entry.isFile()) {
      await onFile(fullPath);
    }
  }
}

async function updatePackageJson(filePath: string, version: string): Promise<void> {
  const raw = await fs.readFile(filePath, 'utf8');
  let parsed: unknown;

  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`Failed to parse JSON in ${relative(filePath)}: ${(error as Error).message}`);
  }

  if (!parsed || typeof parsed !== 'object') {
    throw new Error(`Unexpected JSON shape in ${relative(filePath)} (expected object)`);
  }

  const pkg = parsed as { version?: unknown; [key: string]: unknown };

  if (typeof pkg.version !== 'string') {
    console.warn(`Skipping ${relative(filePath)} (no version field)`);
    return;
  }

  pkg.version = version;

  const newline = raw.includes('\r\n') ? '\r\n' : '\n';
  let serialized = JSON.stringify(pkg, null, 2);
  serialized = serialized.replace(/\n/g, newline) + newline;

  await fs.writeFile(filePath, serialized, 'utf8');
  console.log(`Updated ${relative(filePath)} to ${version}`);
}

async function updateCargoToml(filePath: string, version: string): Promise<void> {
  const raw = await fs.readFile(filePath, 'utf8');
  const newline = raw.includes('\r\n') ? '\r\n' : '\n';
  const lines = raw.split(/\r?\n/);
  const hadFinalNewline = raw.endsWith('\n') || raw.endsWith('\r\n');

  let inPackageSection = false;
  let updated = false;

  const updatedLines = lines.map(line => {
    const trimmed = line.trim();

    if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
      inPackageSection = trimmed === '[package]';
      return line;
    }

    if (inPackageSection) {
      const match = line.match(/^(\s*version\s*=\s*")([^\"]*)(".*)$/);
      if (match) {
        updated = true;
        return `${match[1]}${version}${match[3]}`;
      }
    }

    return line;
  });

  if (!updated) {
    console.warn(`Skipping ${relative(filePath)} (no [package] version field found)`);
    return;
  }

  let content = updatedLines.join(newline);
  if (hadFinalNewline && !content.endsWith(newline)) {
    content += newline;
  }

  await fs.writeFile(filePath, content, 'utf8');
  console.log(`Updated ${relative(filePath)} to ${version}`);
}

async function runCommand(command: string, args: string[], cwd: string): Promise<void> {
  console.log(`Running ${command} ${args.join(' ')} in ${relative(cwd)}`);

  await new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: 'inherit',
      env: process.env
    });

    child.on('error', reject);
    child.on('exit', code => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} ${args.join(' ')} exited with code ${code}`));
      }
    });
  });
}

function relative(filePath: string): string {
  return path.relative(repoRoot, filePath) || '.';
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
