const test = require('node:test'),
  assert = require('node:assert/strict'),
  fs = require('node:fs'),
  vm = require('node:vm'),
  path = require('node:path'),
  ts = require('typescript'),
  { parseHTML } = require('linkedom');
const settle = () => new Promise((r) => setTimeout(r, 25));
function harness() {
  const { window, document } = parseHTML('<html><body><section id="root"></section></body></html>');
  const copy = (v) => JSON.parse(JSON.stringify(v));
  let profile = {
    revision: 1,
    config: {
      workspace: '/tmp/native-work',
      svnDir: '/tmp/svn-wc',
      git: '',
      svn: '',
      svnUsername: 'user',
      svnCredentialFile: '',
      gitName: '',
      gitEmail: '',
      svnTimeout: 60,
      commandTimeout: 300,
      repositories: [
        {
          name: 'repo',
          branch: 'main',
          mergeBranch: 'feature',
          url: '/tmp/remote.git',
          directory: 'project',
        },
      ],
    },
  };
  let calls = [],
    confirmation = true,
    run = null,
    back = false;
  const invoke = async (name, args = {}) => {
    calls.push({ name, args: copy(args) });
    if (name === 'plugins_sync_profile') return copy(profile);
    if (name === 'plugins_sync_history') return run ? [copy(run)] : [];
    if (name === 'plugins_sync_save') {
      if (JSON.stringify(profile.config) !== JSON.stringify(args.config))
        profile = { revision: profile.revision + 1, config: copy(args.config) };
      return copy(profile);
    }
    if (name === 'plugins_sync_import')
      return {
        config: copy(profile.config),
        password: 'FAKE-SESSION-PASS',
        warnings: ['口令仅本次会话'],
      };
    if (name === 'timer_pick') return '/tmp/import';
    if (name === 'timer_confirm') return confirmation;
    if (name === 'plugins_sync_check')
      return { passed: true, checks: [{ name: 'SVN', detail: 'versioned archive' }] };
    if (name === 'plugins_sync_start') {
      run = {
        id: 'run-one',
        pluginId: 'test.sync',
        status: 'running',
        startedAt: '2026-11-22T00:00:00Z',
        finishedAt: null,
        logs: ['<script>unsafe()</script>'],
        repositories: [
          {
            name: 'repo',
            directory: 'project',
            status: 'running',
            gitPush: 'unknown',
            svnCommit: 'not_started',
            archive: '',
            message: '',
          },
        ],
        message: 'running',
        configRevision: profile.revision,
      };
      return run.id;
    }
    if (name === 'plugins_sync_detail') return copy(run);
    if (name === 'plugins_sync_cancel') {
      run.status = 'cancelled';
      return;
    }
    if (name === 'plugins_sync_schedule') return { name: args.options.name, enabled: false };
    throw Error(name);
  };
  const exports = {};
  const source = ts.transpileModule(fs.readFileSync(path.join(__dirname, 'sync-view.ts'), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const req = (name) =>
    name === '@tauri-apps/api/core'
      ? { invoke }
      : {
          escapeHTML: (v) =>
            String(v).replace(
              /[&<>"']/g,
              (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
            ),
        };
  vm.runInNewContext(`(function(require,exports){${source}\n})`, {
    console,
    setInterval: () => 1,
    clearInterval() {},
    navigator: { clipboard: { writeText: async () => {} } },
  })(req, exports);
  const root = document.querySelector('#root');
  const view = new exports.SyncView(
    root,
    'test.sync',
    'Native sync',
    () => {
      back = true;
    },
    {
      perRepository: [
        'git.clone',
        'git.merge-if-configured',
        'git.push',
        'zip.archive',
        'fs.cleanup-clone',
      ],
      afterRepositories: ['svn.commit-archives'],
    },
  );
  const click = (selector) => {
    const el = root.querySelector(selector);
    assert.ok(el, selector);
    el.dispatchEvent(new window.Event('click', { bubbles: true }));
  };
  const value = (selector, v) => {
    const el = root.querySelector(selector);
    assert.ok(el, selector);
    el.value = v;
    el.dispatchEvent(new window.Event('input', { bubbles: true }));
  };
  return {
    view,
    root,
    calls,
    click,
    value,
    setConfirmation: (v) => (confirmation = v),
    get back() {
      return back;
    },
  };
}
test('sync import, no password persistence, explicit start, escaped logs, cancel and paused scheduling', async () => {
  const h = harness();
  try {
    await settle();
    h.click('[data-sync="import"]');
    await settle();
    assert.equal(h.root.querySelector('[data-sync-password]').value, 'FAKE-SESSION-PASS');
    h.click('[data-sync="save"]');
    await settle();
    const save = h.calls.find((c) => c.name === 'plugins_sync_save');
    assert.doesNotMatch(JSON.stringify(save.args.config), /FAKE-SESSION-PASS/);
    h.click('[data-sync="check"]');
    await settle();
    assert.match(h.root.textContent, /未向 Git\/SVN 推送或提交/);
    h.setConfirmation(false);
    h.click('[data-sync="start"]');
    await settle();
    assert.equal(h.calls.filter((c) => c.name === 'plugins_sync_start').length, 0);
    h.setConfirmation(true);
    h.click('[data-sync="start"]');
    await settle();
    assert.equal(
      h.calls.find((c) => c.name === 'plugins_sync_start').args.password,
      'FAKE-SESSION-PASS',
    );
    assert.equal(h.root.querySelector('[data-sync-password]').value, '');
    assert.match(h.root.querySelector('.sync-log').textContent, /<script>/);
    assert.equal(h.root.querySelector('script'), null);
    h.click('[data-sync="cancel"]');
    await settle();
    assert.equal(h.calls.find((c) => c.name === 'plugins_sync_cancel').args.runId, 'run-one');
    h.click('[data-sync="refresh"]');
    await settle();
    assert.match(h.root.textContent, /已取消/);
    h.click('[data-sync="schedule"]');
    await settle();
    const schedule = h.calls.find((c) => c.name === 'plugins_sync_schedule');
    assert.equal(schedule.args.revision, 1);
    assert.equal(schedule.args.options.cron, '0 9 * * 1-5');
    assert.match(h.root.textContent, /已创建暂停计划/);
  } finally {
    h.view.dispose();
  }
});
test('unsaved sync configuration participates in tab and back navigation guard', async () => {
  const h = harness();
  try {
    await settle();
    h.value('[data-sync-field="svnUsername"]', 'changed');
    assert.equal(h.view.dirty(), true);
    h.setConfirmation(false);
    assert.equal(await h.view.canClose(), false);
    h.click('[data-sync="back"]');
    await settle();
    assert.equal(h.back, false);
    h.setConfirmation(true);
    h.click('[data-sync="back"]');
    await settle();
    assert.equal(h.back, true);
    assert.equal(h.view.dirty(), true);
  } finally {
    h.view.dispose();
  }
});
