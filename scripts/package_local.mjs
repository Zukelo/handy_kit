/** Build the host platform's debug package and collect it under builds/. */
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { dirname, join, resolve, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const desktop = join(root, 'development/apps/desktop');
const env = { ...process.env };
// Use the same Node that launched this script and prefer the project's rustup toolchain.
const rustup = spawnSync('rustup', ['which', '--toolchain', 'stable', 'cargo'], { encoding: 'utf8' });
const bins = [dirname(process.execPath)];
if (rustup.status === 0) bins.push(dirname(rustup.stdout.trim()));
env.PATH = [...bins, env.PATH ?? ''].join(delimiter);
const options = process.platform === 'darwin' ? ['--bundles', 'app'] : process.platform === 'win32' ? ['--bundles', 'nsis'] : ['--bundles', 'deb,appimage'];
const npmPath = process.env.npm_execpath;
if (!npmPath) throw new Error('Run with npm run package:local from the desktop project');
const result = spawnSync(process.execPath, [npmPath, 'run', 'package:dev', '--', ...options, '--', '--locked', ...process.argv.slice(2)], { cwd: desktop, env, stdio: 'inherit' });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const bundle = join(desktop, 'src-tauri/target/debug/bundle');
const destination = join(root, 'builds', `${process.platform}-${process.arch}`);
mkdirSync(destination, { recursive: true });
if (process.platform === 'darwin') {
  const source = join(bundle, 'macos/HandyKit.app');
  if (!existsSync(source)) throw new Error('Expected HandyKit.app was not generated');
  const output = join(destination, 'HandyKit.app');
  const copied = spawnSync('ditto', [source, output], { stdio: 'inherit' });
  if (copied.status !== 0) throw new Error('Could not copy HandyKit.app');
  console.log(`\nLocal development app: ${output}`);
} else {
  const extensions = process.platform === 'win32' ? ['.exe'] : ['.deb', '.AppImage'];
  const packages = readdirSync(bundle, { recursive: true, withFileTypes: true }).filter(entry => entry.isFile() && extensions.some(extension => entry.name.endsWith(extension)));
  if (!packages.length) throw new Error('Expected development installers were not generated');
  for (const entry of packages) {
    const output = join(destination, entry.name);
    cpSync(join(entry.parentPath, entry.name), output);
    console.log(`Local development package: ${output}`);
  }
}
