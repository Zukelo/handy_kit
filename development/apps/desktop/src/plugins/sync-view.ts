import { invoke } from '@tauri-apps/api/core';
import { escapeHTML as e } from '../components/html.ts';
import type {
  SyncRepository as Repo,
  SyncConfig as Config,
  SyncProfile as Profile,
  Workflow,
  SyncRun as Run,
} from './model.ts';
const initial = (): Config => ({
  workspace: '',
  svnDir: '',
  git: '',
  svn: '',
  svnUsername: '',
  svnCredentialFile: '',
  gitName: '',
  gitEmail: '',
  svnTimeout: 60,
  commandTimeout: 300,
  repositories: [],
});
const status = (value: string) =>
  ({
    running: '执行中',
    pending: '未开始',
    archived: '已归档',
    succeeded: '成功',
    failed: '失败',
    cancelled: '已取消',
    interrupted: '已中断',
    not_started: '未开始',
    not_requested: '工作流未要求',
    unknown: '结果待核对',
    confirmed: '命令已成功',
  })[value] || value;
export class SyncView {
  private profile: Profile = { revision: 0, config: initial() };
  private baseline = '';
  private password = '';
  private busy = true;
  private disposed = false;
  private error = '';
  private pollError = '';
  private note = '';
  private checks = '';
  private run: Run | null = null;
  private history: Run[] = [];
  private polling = false;
  private timer: ReturnType<typeof setInterval>;
  private schedule = {
    name: 'Git → ZIP → SVN 同步',
    cron: '0 9 * * 1-5',
    zone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC',
    timeout: 3600,
  };
  constructor(
    private root: HTMLElement,
    private id: string,
    private name: string,
    private back: () => void,
    private workflow: Workflow,
  ) {
    root.addEventListener('click', this.click);
    root.addEventListener('input', this.input);
    this.render();
    this.timer = setInterval(() => void this.poll(), 700);
    void this.load();
  }
  dirty() {
    return this.baseline !== '' && JSON.stringify(this.profile.config) !== this.baseline;
  }
  async canClose() {
    if (!this.dirty()) return true;
    const allowed = await invoke<boolean>('timer_confirm', {
      message: '同步配置尚未保存，离开将放弃修改。是否继续？',
    });
    return allowed;
  }
  dispose() {
    this.disposed = true;
    clearInterval(this.timer);
    this.root.removeEventListener('click', this.click);
    this.root.removeEventListener('input', this.input);
  }
  private api<T>(op: string, args: Record<string, unknown> = {}): Promise<T> {
    return invoke<T>(`plugins_sync_${op}`, { id: this.id, ...args });
  }
  private async load() {
    try {
      this.profile = await this.api<Profile>('profile');
      this.baseline = JSON.stringify(this.profile.config);
      this.history = await this.api<Run[]>('history');
      this.run = this.history[0] || null;
    } catch (err) {
      this.error = String(err);
    }
    this.busy = false;
    this.render();
  }
  private async poll() {
    if (this.disposed || this.polling || !this.run || this.run.status !== 'running') return;
    this.polling = true;
    try {
      this.pollError = '';
      this.run = await this.api<Run>('detail', { runId: this.run.id });
      if (this.run.status !== 'running') this.history = await this.api<Run[]>('history');
      this.renderRun();
    } catch (err) {
      this.pollError = String(err);
      this.renderRun();
    } finally {
      this.polling = false;
    }
  }
  private field(key: Exclude<keyof Config, 'repositories'>, label: string, picker = false) {
    const value = this.profile.config[key];
    return `<label>${label}<div class="sync-input"><input data-sync-field="${key}" type="${typeof value === 'number' ? 'number' : 'text'}" value="${e(String(value))}" ${this.busy ? 'disabled' : ''}>${picker ? `<button class="button" data-sync="pick" data-field="${key}" ${this.busy ? 'disabled' : ''}>选择…</button>` : ''}</div></label>`;
  }
  private get steps() {
    return this.workflow.perRepository;
  }
  private render() {
    if (this.disposed) return;
    const c = this.profile.config;
    this.root.innerHTML = `<div class="page-breadcrumb">插件管理 / ${e(this.name)}</div><div class="plugin-header"><div><h1>${e(this.name)}</h1><p>原生流程 · 不执行 sh · 已保存配置 v${this.profile.revision} <span class="sync-dirty">${this.dirty() ? '有未保存修改' : ''}</span></p></div><button class="button" data-sync="back">返回插件列表</button></div>${this.error ? `<p class="plugin-error" role="alert">${e(this.error)}</p>` : ''}${this.note ? `<p role="status">${e(this.note)}</p>` : ''}<section class="plugin-card"><h2>同步配置</h2><p>插件工作流：${this.steps
      .map(
        (step) =>
          ({
            'git.clone': '克隆',
            'git.merge-if-configured': '按配置合并',
            'git.push': '推送',
            'zip.archive': 'ZIP 归档',
            'fs.cleanup-clone': '清理克隆',
          })[step] || step,
      )
      .map(e)
      .join(
        ' → ',
      )}；全部仓库后提交 SVN 归档。</p><div class="plugin-actions"><button class="button" data-sync="import" ${this.busy ? 'disabled' : ''}>导入 config.sh + repository.ini…</button><button class="button" data-sync="save" ${this.busy ? 'disabled' : ''}>保存配置</button><button class="button" data-sync="check" ${this.busy ? 'disabled' : ''}>执行前检查（不推送）</button></div><div class="sync-fields">${this.field('workspace', '工作目录（原脚本目录）', true)}${this.field('svnDir', 'SVN 工作副本目录', true)}${this.field('git', 'Git 程序（留空使用 PATH）', true)}${this.field('svn', 'SVN 程序（留空使用 PATH）', true)}${this.field('svnUsername', 'SVN 用户名（可留空使用已有认证）')}${this.field('svnCredentialFile', '口令变量文件（可选，静态 svn_password）', true)}<label>SVN 口令（仅本次会话，不保存）<input data-sync-password type="password" value="${e(this.password)}" autocomplete="off"></label>${this.field('gitName', 'Git 提交姓名（可选，本轮仓库内生效）')}${this.field('gitEmail', 'Git 提交邮箱（可选）')}${this.field('svnTimeout', 'SVN 网络超时（秒）')}${this.field('commandTimeout', '每条命令超时（秒）')}</div><p>配置保存于本机用户目录；仅保存口令文件路径，不复制其中口令。凭据文件仍由你管理。URL 不应包含口令或令牌。手动执行可输入一次性口令，定时执行可静态读取上述口令文件，或使用已有 Git/SVN 认证缓存。</p><h3>仓库清单</h3>${c.repositories
      .map(
        (r, i) =>
          `<fieldset class="sync-repo"><legend>${e(r.name || `仓库 ${i + 1}`)}</legend><div class="sync-fields">${(
            [
              ['name', '名称'],
              ['branch', '目标分支'],
              ['mergeBranch', '合并分支（可选）'],
              ['url', 'Git URL'],
              ['directory', '相对克隆目录 / ZIP 名称'],
            ] as const
          )
            .map(
              ([key, label]) =>
                `<label>${label}<input data-sync-repo="${i}" data-key="${key}" value="${e(r[key])}" ${this.busy ? 'disabled' : ''}></label>`,
            )
            .join(
              '',
            )}</div><button class="button" data-sync="remove" data-index="${i}" ${this.busy ? 'disabled' : ''}>移除此仓库</button></fieldset>`,
      )
      .join(
        '',
      )}<button class="button" data-sync="add" ${this.busy ? 'disabled' : ''}>＋ 添加仓库</button></section><section class="plugin-card"><h2>执行同步</h2><p>${this.steps.includes('git.push') ? '会向配置的 Git 远端 push，' : '此插件工作流不要求 Git push；'}覆盖 SVN 工作副本中对应的已版本化 ZIP 并 commit。不会自动 svn add/update；已有本地克隆目录或 ZIP 会阻止执行。ZIP 包含隐藏文件和 .git；遇到符号链接会停止并保留现场。</p><button class="button primary" data-sync="start" ${this.busy || this.run?.status === 'running' ? 'disabled' : ''}>保存配置并执行同步…</button><p>${e(this.checks)}</p></section><section class="plugin-card"><h2>定时执行</h2><p>创建为暂停计划，在「定时任务」中确认后启用。计划绑定已保存配置版本；修改插件配置后须更新计划版本。定时运行仅引用静态口令文件或已有认证缓存，不把口令存入计划。启用新计划前，请停用原 sh 计划，避免重复同步。</p><div class="sync-fields">${(['name', 'cron', 'zone', 'timeout'] as const).map((key, i) => `<label>${['计划名称', 'Cron（五字段）', 'IANA 时区', '本轮总超时（秒）'][i]}<input data-sync-schedule="${key}" value="${e(String(this.schedule[key]))}" type="${key === 'timeout' ? 'number' : 'text'}"></label>`).join('')}</div><button class="button" data-sync="schedule" ${this.busy ? 'disabled' : ''}>保存配置并创建暂停计划</button></section><section class="sync-run" aria-live="polite"></section>`;
    this.renderRun();
  }
  private renderRun() {
    if (this.disposed) return;
    const root = this.root.querySelector('.sync-run');
    if (!root) return;
    const r = this.run;
    const previous = root.querySelector<HTMLElement>('.sync-log');
    const scroll = previous
      ? {
          top: previous.scrollTop,
          follow: previous.scrollHeight - previous.clientHeight - previous.scrollTop < 32,
        }
      : null;
    root.innerHTML = `${this.pollError ? `<p class="plugin-error" role="alert">${e(this.pollError)}</p>` : ''}<div class="plugin-header"><h2>执行记录</h2><button class="button" data-sync="refresh">刷新</button></div>${this.history.map((h) => `<button class="button" data-sync="history" data-run-id="${e(h.id)}">${e(h.startedAt)} · ${status(h.status)}</button>`).join('')}${r ? `<article class="plugin-card"><div class="plugin-header"><strong>${status(r.status)} · 配置 v${r.configRevision}</strong>${r.status === 'running' ? '<button class="button" data-sync="cancel">停止同步</button>' : '<button class="button" data-sync="copy">复制执行报告</button>'}</div><p>${e(r.message)}</p>${r.repositories.map((o) => `<div class="plugin-card"><strong>${e(o.name)} · ${status(o.status)}</strong><p>Git push：${status(o.gitPush)} · SVN commit：${status(o.svnCommit)}</p><p>${e(o.message)}</p>${o.gitCommit ? `<small>Git HEAD：${e(o.gitCommit)}</small>` : ''}${o.archive ? `<small>本地归档路径（成功后可能已清理）：${e(o.archive)}</small>` : ''}</div>`).join('')}<pre class="sync-log">${e(r.logs.join('\n'))}</pre></article>` : '<p>暂无执行记录。</p>'}`;
    const current = root.querySelector<HTMLElement>('.sync-log');
    if (current) current.scrollTop = !scroll || scroll.follow ? current.scrollHeight : scroll.top;
  }
  private async save() {
    this.profile = await this.api<Profile>('save', {
      revision: this.profile.revision,
      config: this.profile.config,
    });
    this.baseline = JSON.stringify(this.profile.config);
    this.note = `配置 v${this.profile.revision} 已保存，SVN 口令未保存。配置变更会暂停原计划，请在定时任务中更新版本后启用。`;
  }
  private input = (event: Event) => {
    const el = event.target as HTMLInputElement;
    const key = el.dataset.syncField as Exclude<keyof Config, 'repositories'> | undefined;
    if (key) {
      if (key === 'svnTimeout' || key === 'commandTimeout')
        this.profile.config[key] = Number(el.value);
      else this.profile.config[key] = el.value;
    }
    if (el.hasAttribute('data-sync-password')) this.password = el.value;
    if (el.dataset.syncRepo !== undefined) {
      const row = this.profile.config.repositories[Number(el.dataset.syncRepo)];
      row[el.dataset.key as keyof Repo] = el.value;
    }
    const s = el.dataset.syncSchedule as keyof typeof this.schedule | undefined;
    if (s) {
      if (s === 'timeout') this.schedule.timeout = Number(el.value);
      else this.schedule[s] = el.value;
    }
    const dirty = this.root.querySelector('.sync-dirty');
    if (dirty) dirty.textContent = this.dirty() ? '有未保存修改' : '';
  };
  private click = async (event: Event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-sync]');
    if (!button) return;
    const action = button.dataset.sync!;
    if (action === 'cancel') {
      try {
        await this.api('cancel', { runId: this.run!.id });
      } catch (err) {
        this.error = String(err);
        this.render();
      }
      return;
    }
    if (action === 'copy') {
      try {
        await navigator.clipboard.writeText(JSON.stringify(this.run, null, 2));
      } catch (err) {
        this.error = String(err);
        this.render();
      }
      return;
    }
    if (this.busy) return;
    if (action === 'back') {
      if (await this.canClose()) {
        this.dispose();
        this.back();
      }
      return;
    }
    this.busy = true;
    this.error = '';
    this.render();
    try {
      if (action === 'pick') {
        const key = button.dataset.field as Exclude<
          keyof Config,
          'repositories' | 'svnTimeout' | 'commandTimeout'
        >;
        const path = await invoke<string | null>('timer_pick', {
          kind: key === 'workspace' || key === 'svnDir' ? 'folder' : 'target',
        });
        if (path) this.profile.config[key] = path;
      }
      if (action === 'import') {
        const directory = await invoke<string | null>('timer_pick', { kind: 'folder' });
        if (directory) {
          const result = await this.api<{ config: Config; password: string; warnings: string[] }>(
            'import',
            { directory },
          );
          this.profile.config = result.config;
          this.password = result.password;
          this.note = result.warnings.join('；') || '配置已导入，请检查并保存。';
        }
      }
      if (action === 'add')
        this.profile.config.repositories.push({
          name: `repository${this.profile.config.repositories.length + 1}`,
          branch: '',
          mergeBranch: '',
          url: '',
          directory: '',
        });
      if (action === 'remove')
        this.profile.config.repositories.splice(Number(button.dataset.index), 1);
      if (action === 'save') await this.save();
      if (action === 'check') {
        const result = await this.api<{
          passed: boolean;
          checks: { name: string; detail: string }[];
        }>('check', { config: this.profile.config, password: this.password });
        this.checks = result.checks.map((c) => `${c.name}：${c.detail}`).join('\n');
        this.note = '本地执行前检查通过；未向 Git/SVN 推送或提交，远端认证尚未验证。';
      }
      if (action === 'start') {
        await this.save();
        const allowed = await invoke<boolean>('timer_confirm', {
          message: `执行已保存的 v${this.profile.revision} 配置？将处理 ${this.profile.config.repositories.length} 个仓库${this.steps.includes('git.push') ? '并执行 Git push' : ''}，覆盖 SVN 对应归档文件后 commit。失败不自动回滚已完成的远端写入。`,
        });
        if (allowed) {
          const runId = await this.api<string>('start', {
            revision: this.profile.revision,
            password: this.password,
          });
          this.password = '';
          this.run = await this.api<Run>('detail', { runId });
          this.history = await this.api<Run[]>('history');
        }
      }
      if (action === 'schedule') {
        await this.save();
        const task = await this.api<{ name: string }>('schedule', {
          revision: this.profile.revision,
          options: { ...this.schedule },
        });
        this.note = `已创建暂停计划“${task.name}”，请到定时任务确认后启用。`;
      }
      if (action === 'history')
        this.run = await this.api<Run>('detail', { runId: button.dataset.runId });
      if (action === 'refresh') {
        this.history = await this.api<Run[]>('history');
        if (this.run) this.run = await this.api<Run>('detail', { runId: this.run.id });
      }
    } catch (err) {
      this.error = String(err);
    } finally {
      this.busy = false;
      this.render();
    }
  };
}
