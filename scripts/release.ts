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

  const publishableTypeScriptPackages = await getPublishableTypeScriptPackages(manifests.packageJson);
  const publishableRustPackages = await getPublishableRustPackages(manifests.cargoToml);

  await createAndPushCommit(version);

  await publishTypeScriptPackages(publishableTypeScriptPackages, version);
  await publishRustPackages(publishableRustPackages, version);
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

type PublishableTypeScriptPackage = {
  name: string;
  directory: string;
};

async function getPublishableTypeScriptPackages(packageJsonPaths: string[]): Promise<PublishableTypeScriptPackage[]> {
  const tsRoot = path.join(repoRoot, 'typescript');
  const packages: PublishableTypeScriptPackage[] = [];

  for (const filePath of packageJsonPaths) {
    const isTypeScriptPackage =
      filePath === path.join(tsRoot, 'package.json') ||
      filePath.startsWith(`${tsRoot}${path.sep}`);

    if (!isTypeScriptPackage) {
      continue;
    }

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

    const pkg = parsed as { name?: unknown; private?: unknown };

    if (pkg.private === true) {
      continue;
    }

    if (typeof pkg.name !== 'string' || pkg.name.length === 0) {
      console.warn(`Skipping ${relative(filePath)} (missing package name)`);
      continue;
    }

    packages.push({
      name: pkg.name,
      directory: path.dirname(filePath)
    });
  }

  return packages;
}

async function publishTypeScriptPackages(packages: PublishableTypeScriptPackage[], version: string): Promise<void> {
  if (packages.length === 0) {
    console.warn('No publishable TypeScript packages found, skipping npm publish');
    return;
  }

  const tag = version.includes('-rc.') ? 'rc' : 'latest';

  for (const pkg of packages) {
    console.log(`Preparing to publish ${pkg.name}@${version} from ${relative(pkg.directory)}`);

    if (await packageVersionExists(pkg.name, version)) {
      console.log(`Skipping ${pkg.name}@${version} (already published)`);
      continue;
    }

    await runCommand(
      'pnpm',
      ['--filter', pkg.name, 'publish', '--access', 'public', '--tag', tag],
      path.join(repoRoot, 'typescript')
    );
  }

  console.log(`Published TypeScript packages with tag '${tag}'`);
}

async function packageVersionExists(name: string, version: string): Promise<boolean> {
  try {
    await captureCommand('npm', ['view', `${name}@${version}`, 'version'], repoRoot);
    return true;
  } catch {
    return false;
  }
}

type PublishableRustPackage = {
  name: string;
  directory: string;
  localDependencies: string[];
};

async function getPublishableRustPackages(cargoTomlPaths: string[]): Promise<PublishableRustPackage[]> {
  const rustRoot = path.join(repoRoot, 'rust');
  const packages: PublishableRustPackage[] = [];

  for (const filePath of cargoTomlPaths) {
    const normalized = path.normalize(filePath);
    const rustWorkspaceRoot = path.join(rustRoot, 'Cargo.toml');

    if (normalized === rustWorkspaceRoot) {
      continue;
    }

    const isRustCrate = normalized.startsWith(`${rustRoot}${path.sep}`);
    if (!isRustCrate) {
      continue;
    }

    const crateDirectory = path.dirname(normalized);
    const relativeToRustRoot = path.relative(rustRoot, crateDirectory);
    if (relativeToRustRoot.split(path.sep).includes('examples')) {
      continue;
    }

    const raw = await fs.readFile(normalized, 'utf8');
    const lines = raw.split(/\r?\n/);

    let inPackageSection = false;
    let packageName: string | undefined;
    let publishFlag: string | undefined;

    for (const line of lines) {
      const trimmed = line.trim();

      if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
        inPackageSection = trimmed === '[package]';
        continue;
      }

      if (!inPackageSection) {
        continue;
      }

      if (!packageName) {
        const nameMatch = trimmed.match(/^name\s*=\s*"([^"]+)"/);
        if (nameMatch) {
          packageName = nameMatch[1];
          continue;
        }
      }

      if (!publishFlag) {
        const publishMatch = trimmed.match(/^publish\s*=\s*(.+)$/);
        if (publishMatch) {
          publishFlag = publishMatch[1].trim();
        }
      }
    }

    if (publishFlag && publishFlag.startsWith('false')) {
      continue;
    }

    if (!packageName) {
      console.warn(`Skipping ${relative(normalized)} (missing package name)`);
      continue;
    }

    const localDependencies: string[] = [];
    const dependencyRegex = /^([A-Za-z0-9_-]+)\s*=\s*{[^}]*path\s*=\s*"([^"]+)"[^}]*}$/gm;
    let match: RegExpExecArray | null;
    while ((match = dependencyRegex.exec(raw)) !== null) {
      localDependencies.push(match[1]);
    }

    packages.push({
      name: packageName,
      directory: crateDirectory,
      localDependencies
    });
  }

  return packages;
}

async function publishRustPackages(packages: PublishableRustPackage[], version: string): Promise<void> {
  if (packages.length === 0) {
    console.warn('No publishable Rust crates found, skipping cargo publish');
    return;
  }

  const ordered = orderRustPackages(packages);

  for (const pkg of ordered) {
    console.log(`Preparing to publish ${pkg.name}@${version} from ${relative(pkg.directory)}`);

    if (await rustPackageVersionExists(pkg.name, version)) {
      console.log(`Skipping ${pkg.name}@${version} (already published)`);
      continue;
    }

    await runCommand('cargo', ['publish'], pkg.directory);
  }

  console.log('Published Rust crates');
}

function orderRustPackages(packages: PublishableRustPackage[]): PublishableRustPackage[] {
  const ordered: PublishableRustPackage[] = [];
  const packageMap = new Map(packages.map(pkg => [pkg.name, pkg]));
  const visited = new Set<string>();
  const visiting = new Set<string>();

  const visit = (pkg: PublishableRustPackage): void => {
    if (visited.has(pkg.name)) {
      return;
    }

    if (visiting.has(pkg.name)) {
      console.warn(`Detected cyclic dependency while ordering ${pkg.name}`);
      return;
    }

    visiting.add(pkg.name);

    for (const dep of pkg.localDependencies) {
      const depPkg = packageMap.get(dep);
      if (depPkg) {
        visit(depPkg);
      }
    }

    visiting.delete(pkg.name);
    visited.add(pkg.name);
    ordered.push(pkg);
  };

  for (const pkg of packages) {
    visit(pkg);
  }

  return ordered;
}

async function rustPackageVersionExists(name: string, version: string): Promise<boolean> {
  try {
    const output = await captureCommand('cargo', ['search', name, '--limit', '1'], repoRoot);
    return output
      .split(/\r?\n/)
      .some(line => line.startsWith(`${name} = "${version}"`));
  } catch {
    return false;
  }
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

async function createAndPushCommit(version: string): Promise<void> {
  if (!(await hasPendingChanges())) {
    console.warn('No changes detected, skipping commit and push');
    return;
  }

  await runCommand('git', ['add', '--all'], repoRoot);
  await runCommand('git', ['commit', '-m', `chore: release ${version}`], repoRoot);
  await runCommand('git', ['push'], repoRoot);
}

async function hasPendingChanges(): Promise<boolean> {
  const output = await captureCommand('git', ['status', '--porcelain'], repoRoot);
  return output.trim().length > 0;
}

async function captureCommand(command: string, args: string[], cwd: string): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: ['ignore', 'pipe', 'inherit'],
      env: process.env
    });

    let output = '';

    child.stdout?.on('data', chunk => {
      output += chunk.toString();
    });

    child.on('error', reject);
    child.on('close', code => {
      if (code === 0) {
        resolve(output);
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
