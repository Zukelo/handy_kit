/** Optional macOS preview app. Does not replace the production HandyKit.app. */
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'darwin') throw new Error('Open docs/prototypes/tasks/index.html on this platform; this preview packager targets macOS.');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const desktop = join(root, 'development/apps/desktop');
const temporary = mkdtempSync(join(tmpdir(), 'handykit-tasks-preview-'));
const config = join(temporary, 'preview.json');
const name = 'HandyKit Tasks Prototype';
const env = { ...process.env };
const rustup = spawnSync('rustup', ['which', '--toolchain', 'stable', 'cargo'], { encoding: 'utf8' });
env.PATH = [dirname(process.execPath), ...(rustup.status === 0 ? [dirname(rustup.stdout.trim())] : []), env.PATH ?? ''].join(delimiter);
writeFileSync(config, JSON.stringify({
  productName: name,
  identifier: 'dev.handykit.tasks.prototype',
  build: { frontendDist: join(root, 'docs/prototypes/tasks'), beforeBuildCommand: 'node -e "process.exit(0)"' },
  app: {
    windows: [{ label: 'main', title: '小工坊 · 定时任务设计原型', width: 1380, height: 900, minWidth: 1000, minHeight: 700 }],
    security: { csp: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost" },
  },
}), 'utf8');
try {
  const result = spawnSync(process.execPath, [join(desktop, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', '--debug', '--bundles', 'app', '--config', config, '--', '--locked', ...process.argv.slice(2)], { cwd: desktop, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exitCode = result.status ?? 1;
  else {
    const destination = join(root, 'builds/prototypes', `${name}.app`);
    mkdirSync(dirname(destination), { recursive: true });
    const copy = spawnSync('ditto', [join(desktop, 'src-tauri/target/debug/bundle/macos', `${name}.app`), destination], { stdio: 'inherit' });
    if (copy.status !== 0) throw new Error('Could not copy the preview app');
    console.log(`\nPrototype preview: ${destination}`);
  }
} finally { rmSync(temporary, { recursive: true, force: true }); }
