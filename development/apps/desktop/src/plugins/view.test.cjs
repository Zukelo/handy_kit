const test = require('node:test'),
  assert = require('node:assert/strict'),
  fs = require('node:fs'),
  path = require('node:path'),
  vm = require('node:vm'),
  ts = require('typescript'),
  { parseHTML } = require('linkedom');
const settle = () => new Promise((r) => setTimeout(r, 20));
function harness(native = true) {
  const { window, document } = parseHTML('<html><body><section id="root"></section></body></html>');
  let items = [],
    calls = [];
  const manifest = {
    id: 'example.plugin',
    version: '1.0.0',
    name: '<script>Plugin</script>',
    description: 'Diagnostics',
    operations: ['git.version', 'svn.version', 'git.local-roundtrip'],
  };
  const invoke = async (name, args) => {
    calls.push({ name, args });
    if (name === 'plugins_list') return JSON.parse(JSON.stringify(items));
    if (name === 'plugins_install') {
      items = [{ manifest, enabled: true }];
      return manifest;
    }
    if (name === 'plugins_toggle') {
      items[0].enabled = args.enabled;
      return;
    }
    if (name === 'plugins_uninstall') {
      items = [];
      return;
    }
    if (name === 'timer_confirm') return true;
    if (name === 'timer_pick') return '/tmp/workspace';
    if (name === 'plugins_run')
      return {
        pluginId: manifest.id,
        pluginVersion: '1.0.0',
        platform: 'test',
        scope: 'Not remote validation',
        checks: [
          { name: 'Git', status: 'failed', detail: '<img src=x onerror=bad()> tool missing' },
        ],
      };
    throw Error(name);
  };
  const exports = {};
  const source = ts.transpileModule(fs.readFileSync(path.join(__dirname, 'view.ts'), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const req = (n) =>
    n === '@tauri-apps/api/core'
      ? { invoke, isTauri: () => native }
      : n.endsWith('html.ts')
        ? {
            escapeHTML: (v) =>
              String(v).replace(
                /[&<>"']/g,
                (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
              ),
          }
        : {};
  vm.runInNewContext(`(function(require,exports){${source}\n})`, {
    console,
    navigator: { clipboard: { writeText: async () => {} } },
  })(req, exports);
  const root = document.querySelector('#root');
  const view = new exports.PluginsView(root);
  const click = (selector) => {
    const el = root.querySelector(selector);
    assert.ok(el, selector);
    el.dispatchEvent(new window.Event('click', { bubbles: true }));
  };
  return { root, view, calls, click, window };
}
test('independent plugin installation, disable, probe, uninstall and escaped reports', async () => {
  const h = harness();
  try {
    await settle();
    assert.match(h.root.textContent, /尚未安装/);
    h.click('[data-plugin="install"]');
    await settle();
    assert.match(h.root.textContent, /<script>Plugin/);
    assert.equal(h.root.querySelector('script'), null);
    h.click('[data-plugin="pick"][data-field="workspace"]');
    await settle();
    h.click('[data-plugin="run"]');
    await settle();
    assert.equal(h.calls.find((c) => c.name === 'plugins_run').args.settings.localRoundtrip, false);
    assert.match(h.root.textContent, /tool missing/);
    assert.equal(h.root.querySelector('img'), null);
    h.click('[data-plugin="roundtrip"]');
    await settle();
    assert.equal(
      h.calls.filter((c) => c.name === 'plugins_run').at(-1).args.settings.localRoundtrip,
      true,
    );
    h.click('[data-plugin="toggle"]');
    await settle();
    assert.equal(h.root.querySelector('[data-plugin="run"]').hasAttribute('disabled'), true);
    h.click('[data-plugin="uninstall"]');
    await settle();
    assert.match(h.root.textContent, /尚未安装/);
  } finally {
    h.view.dispose();
  }
});
test('browser preview never invokes native plugin commands and disposal removes handlers', async () => {
  const h = harness(false);
  await settle();
  assert.equal(h.calls.length, 0);
  assert.match(h.root.textContent, /本机小工坊/);
  h.view.dispose();
  h.click('[data-plugin="install"]');
  await settle();
  assert.equal(h.calls.length, 0);
});
