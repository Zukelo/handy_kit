import { invoke, isTauri } from '@tauri-apps/api/core';
import { escapeHTML as e } from '../components/html.ts';
import './plugins.css';
import { SyncView } from './sync-view.ts';
import type { InstalledPlugin as Installed, PluginReport as Report } from './model.ts';
export class PluginsView {
  private sync: SyncView | null = null;
  dirty() {
    return this.sync?.dirty() || false;
  }
  async canClose() {
    return this.sync ? this.sync.canClose() : true;
  }
  private items: Installed[] = [];
  private error = '';
  private busy = false;
  private disposed = false;
  private report: Report | null = null;
  private settings = {
    git: '',
    svn: '',
    svnadmin: '',
    workspace: '',
    testFile: '',
    localRoundtrip: false,
  };
  constructor(private root: HTMLElement) {
    root.classList.add('plugins-view');
    root.addEventListener('click', this.click);
    root.addEventListener('input', this.input);
    this.render();
    void this.load();
  }
  dispose() {
    this.sync?.dispose();
    this.disposed = true;
    this.root.removeEventListener('click', this.click);
    this.root.removeEventListener('input', this.input);
  }
  private api<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
    if (!isTauri()) return Promise.reject(new Error('插件需在本机小工坊中使用。'));
    return invoke<T>(`plugins_${name}`, args);
  }
  private async load() {
    try {
      this.items = await this.api<Installed[]>('list');
    } catch (err) {
      this.error = String(err);
    }
    if (!this.disposed) this.render();
  }
  private input = (event: Event) => {
    const el = event.target as HTMLInputElement;
    const key = el.dataset.setting as
      | keyof Omit<typeof this.settings, 'localRoundtrip'>
      | undefined;
    if (key) this.settings[key] = el.value;
  };
  private field(
    key: keyof Omit<typeof this.settings, 'localRoundtrip'>,
    label: string,
    hint: string,
  ) {
    return `<label>${label}<input data-setting="${key}" value="${e(this.settings[key])}" placeholder="${e(hint)}" ${this.busy ? 'disabled' : ''}><button type="button" class="button" data-plugin="pick" data-field="${key}" ${this.busy ? 'disabled' : ''}>选择…</button></label>`;
  }
  private render() {
    if (this.disposed || this.sync) return;
    this.root.innerHTML = `<div class="page-breadcrumb">工具箱 / 插件管理</div><div class="plugin-header"><div><h1>插件管理</h1><p>独立安装、停用与卸载；能力由插件包声明。</p></div><button class="button primary" data-plugin="install" ${this.busy ? 'disabled' : ''}>安装插件包…</button></div>${this.error ? `<p class="plugin-error" role="alert">${e(this.error)}</p>` : ''}<div class="plugin-layout"><section><h2>已安装插件</h2>${this.items.map((i) => `<article class="plugin-card"><h3>${e(i.manifest.name)} <small>${e(i.manifest.version)}</small></h3><p>${e(i.manifest.description)}</p><small>${e(i.manifest.id)} · ${i.enabled ? '已启用' : '已停用'}</small><p class="plugin-operations">${e(i.manifest.operations.join(' · '))}</p><div class="plugin-actions">${i.manifest.operations.includes('code-sync.run') ? `<button class="button primary" data-plugin="sync" data-id="${e(i.manifest.id)}" ${this.busy || !i.enabled ? 'disabled' : ''}>配置 / 执行同步</button>` : `<button class="button" data-plugin="run" data-id="${e(i.manifest.id)}" ${this.busy || !i.enabled ? 'disabled' : ''}>检查版本与目录</button>${i.manifest.operations.some((op) => op.endsWith('local-roundtrip')) ? `<button class="button" data-plugin="roundtrip" data-id="${e(i.manifest.id)}" ${this.busy || !i.enabled ? 'disabled' : ''}>本地读写自检</button>` : ''}`}<button class="button" data-plugin="toggle" data-id="${e(i.manifest.id)}" ${this.busy ? 'disabled' : ''}>${i.enabled ? '停用' : '启用'}</button><button class="button" data-plugin="uninstall" data-id="${e(i.manifest.id)}" ${this.busy ? 'disabled' : ''}>卸载</button></div></article>`).join('') || '<p class="plugin-card">尚未安装插件。点击“安装插件包”，选择 .hkp 文件。</p>'}</section><aside class="plugin-card"><h2>检查环境</h2>${this.field('workspace', '工作目录', '请选择沙箱内现有的绝对目录')}${this.field('testFile', '已知沙箱文件（可选）', '仅检查小工坊主进程读取；不输出内容')}${this.field('git', 'Git 程序（可选）', '留空从当前 PATH 查找 git.exe')}${this.field('svn', 'SVN 程序（可选）', '留空从当前 PATH 查找 svn.exe')}${this.field('svnadmin', 'svnadmin 程序（本地自检用）', '优先使用 svn 相邻程序，再查找 PATH')}<p>版本与目录检查不写入仓库。本地读写自检会在所选目录创建专用临时仓库，验证成功后清理；失败保留现场。不连接真实远端，不提交业务文件。</p></aside></div><section class="plugin-results" aria-live="polite">${this.busy ? '<p role="status">正在检查，请稍候…</p>' : ''}${this.report ? `<div class="plugin-header"><h2>检查报告 · ${e(this.report.platform)}</h2><button class="button" data-plugin="copy">复制报告</button></div><p>${e(this.report.scope)}</p>${this.report.checks.map((c) => `<article class="plugin-card ${c.status === 'passed' ? 'plugin-pass' : 'plugin-fail'}"><strong>${c.status === 'passed' ? '通过' : '未通过'} · ${e(c.name)}</strong><pre>${e(c.detail)}</pre></article>`).join('')}` : ''}</section>`;
  }
  private click = async (event: Event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-plugin]');
    if (!button || this.busy) return;
    const action = button.dataset.plugin!,
      id = button.dataset.id || '';
    if (action === 'sync') {
      const item = this.items.find((i) => i.manifest.id === id);
      if (!item?.manifest.workflow) {
        this.error = '同步插件缺少工作流定义';
        this.render();
        return;
      }
      this.sync = new SyncView(
        this.root,
        id,
        item.manifest.name,
        () => {
          this.sync = null;
          void this.load();
        },
        item.manifest.workflow,
      );
      return;
    }
    if (action === 'copy') {
      try {
        await navigator.clipboard.writeText(JSON.stringify(this.report, null, 2));
      } catch (err) {
        this.error = String(err);
        this.render();
      }
      return;
    }
    this.busy = true;
    this.error = '';
    this.render();
    try {
      if (action === 'pick') {
        const key = button.dataset.field as keyof Omit<typeof this.settings, 'localRoundtrip'>;
        const path = await invoke<string | null>('timer_pick', {
          kind: key === 'workspace' ? 'folder' : 'target',
        });
        if (path) this.settings[key] = path;
      }
      if (action === 'install') await this.api('install');
      if (action === 'toggle') {
        const item = this.items.find((i) => i.manifest.id === id)!;
        await this.api('toggle', { id, enabled: !item.enabled });
      }
      if (action === 'uninstall') {
        const allowed = await invoke<boolean>('timer_confirm', {
          message: '卸载此插件？仅移除插件包，不删除工作目录和自检失败现场。',
        });
        if (allowed) await this.api('uninstall', { id });
      }
      if (action === 'run' || action === 'roundtrip') {
        this.report = null;
        this.report = await this.api<Report>('run', {
          id,
          settings: { ...this.settings, localRoundtrip: action === 'roundtrip' },
        });
      }
      this.items = await this.api<Installed[]>('list');
    } catch (err) {
      this.error = String(err);
    } finally {
      this.busy = false;
      this.render();
    }
  };
}
