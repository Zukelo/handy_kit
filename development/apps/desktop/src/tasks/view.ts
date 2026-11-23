import { invoke, isTauri } from '@tauri-apps/api/core';
import { escapeHTML as e } from '../components/html.ts';
import {
  freshTask,
  generatedCron,
  visualFields,
  statuses,
  type Task,
  type Overview,
  type Run,
  type Preview,
} from './model.ts';
import './tasks.css';
const kinds: Record<string, string> = {
  script: '运行脚本',
  executable: '运行程序',
  application: '打开应用',
  plugin: '运行原生插件',
};
const options = (pairs: [string, string][], value: string) =>
  pairs
    .map(
      ([v, label]) =>
        `<option value="${e(v)}" ${v === value ? 'selected' : ''}>${e(label)}</option>`,
    )
    .join('');
const button = (label: string, action: string, extra = '') =>
  `<button class="button ${extra}" type="button" data-timer="${action}">${label}</button>`;
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));
export class TasksView {
  private root: HTMLElement;
  private page = 'list';
  private data: Overview = { tasks: [], running: 0, concurrency: 3, stopping: false, notice: '' };
  private platform = '';
  private draft: Task | null = null;
  private baseline = '';
  private step = 0;
  private mode = 'simple';
  private preset = 'weekly';
  private days = [1, 2, 3, 4, 5];
  private period = 5;
  private minute = 0;
  private time = '09:00';
  private monthdays = '1';
  private fields = visualFields('* * * * *')!;
  private preview: Preview | null = null;
  private previewError = '';
  private previewSequence = 0;
  private debounce: ReturnType<typeof setTimeout> | undefined;
  private timer: ReturnType<typeof setInterval>;
  private search = '';
  private kind = 'all';
  private status = 'all';
  private history: Run[] = [];
  private offset = 0;
  private error = '';
  private busy = false;
  private disposed = false;
  private detailId = '';
  private log: Run | null = null;
  private pollBusy = false;
  constructor(root: HTMLElement) {
    this.root = root;
    root.classList.add('tasks-view');
    root.addEventListener('click', this.click);
    root.addEventListener('input', this.input);
    root.addEventListener('change', this.change);
    root.innerHTML = '<p role="status">正在载入本机任务…</p>';
    this.timer = setInterval(() => void this.poll(), 1500);
    void this.load();
  }
  public dirty(): boolean {
    return !!this.draft && JSON.stringify(this.draft) !== this.baseline;
  }
  public async canClose(): Promise<boolean> {
    const allowed =
      !this.dirty() || (await this.confirm('此任务尚未保存。关闭标签将放弃本次修改，是否继续？'));
    return allowed;
  }
  public dispose(): void {
    this.disposed = true;
    clearInterval(this.timer);
    clearTimeout(this.debounce);
    this.root.removeEventListener('click', this.click);
    this.root.removeEventListener('input', this.input);
    this.root.removeEventListener('change', this.change);
  }
  private async api<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    if (!isTauri()) throw new Error('定时任务需在本机小工坊 App 中使用；浏览器预览不会执行程序。');
    return invoke<T>(`timer_${command}`, args);
  }
  private async load() {
    try {
      this.platform = await this.api<string>('platform');
      this.data = await this.api<Overview>('overview');
      this.error = '';
    } catch (err) {
      this.error = String(err);
    }
    if (!this.disposed) this.render();
  }
  private async poll() {
    if (this.disposed || this.pollBusy || !isTauri()) return;
    this.pollBusy = true;
    try {
      this.data = await this.api<Overview>('overview');
      if (this.disposed) return;
      if (this.log) {
        this.log = await this.api<Run>('run_detail', { id: this.log.id });
        this.renderLog();
        return;
      }
      if (this.page === 'list' || this.page === 'detail') this.render();
      if (this.page === 'history') {
        this.history = await this.api<Run[]>('history', {
          taskId: this.detailId || null,
          offset: this.offset,
        });
        this.render();
      }
    } catch (err) {
      this.error = String(err);
    } finally {
      this.pollBusy = false;
    }
  }
  private date(value: string | null, zone?: string): string {
    if (!value) return '—';
    try {
      return new Date(value).toLocaleString('zh-CN', {
        timeZone: zone || Intl.DateTimeFormat().resolvedOptions().timeZone,
        hour12: false,
      });
    } catch {
      return value;
    }
  }
  private badge(status?: string) {
    return status
      ? `<span class="timer-badge ${e(status)}">${e(statuses[status] || status)}</span>`
      : '<span class="timer-muted">尚未执行</span>';
  }
  private render() {
    if (this.disposed) return;
    const content =
      this.page === 'editor'
        ? this.editor()
        : this.page === 'history'
          ? this.historyPage()
          : this.page === 'detail'
            ? this.detail()
            : this.list();
    this.root.innerHTML = `<div class="page-breadcrumb">工具箱 / 时间与任务 / 定时任务</div><div class="timer-header"><div><h1>${this.page === 'editor' ? (this.draft?.id ? '编辑任务' : '新建任务') : this.page === 'history' ? '执行记录' : this.page === 'detail' ? '任务详情' : '定时任务'}</h1><p>本机后台调度 · Unix 五字段 · 当前平台 ${e(this.platform || '载入中')}</p></div><div class="timer-actions">${this.page === 'list' ? button('执行记录', 'history') + button('＋ 新建任务', 'new', 'primary') : button('返回任务列表', 'list')}</div></div>${this.error ? `<div class="timer-error" role="alert">${e(this.error)}</div>` : ''}<p class="timer-notice">${e(this.data.notice || '任务按当前用户身份执行，不提升权限。')} ${this.data.stopping ? '正在等待运行任务结束后退出。' : ''}</p>${content}<div class="timer-dialog-host"></div>`;
    if (this.page === 'editor') this.renderPreview();
    if (this.log) this.renderLog();
  }
  private list() {
    const tasks = this.data.tasks.filter(
      ({ task }) =>
        (!this.search ||
          `${task.name} ${task.description}`.toLowerCase().includes(this.search.toLowerCase())) &&
        (this.kind === 'all' || task.action.kind === this.kind) &&
        (this.status === 'all' || task.enabled === (this.status === 'enabled')),
    );
    return `<div class="timer-stats"><div><strong>${this.data.tasks.length}</strong>全部任务</div><div><strong>${this.data.tasks.filter((i) => i.task.enabled).length}</strong>已启用</div><div><strong>${this.data.running} / ${this.data.concurrency}</strong>执行中</div><div><strong>${this.data.tasks.filter((i) => i.issue).length}</strong>配置需检查</div></div><div class="timer-toolbar"><input aria-label="搜索任务" placeholder="搜索任务名称、说明" data-filter="search" value="${e(this.search)}"><select aria-label="动作筛选" data-filter="kind">${options([['all', '全部动作'], ...Object.entries(kinds)], this.kind)}</select><select aria-label="状态筛选" data-filter="status">${options(
      [
        ['all', '全部状态'],
        ['enabled', '已启用'],
        ['paused', '已暂停'],
      ],
      this.status,
    )}</select></div><div class="timer-table-scroll"><table class="timer-table"><thead><tr><th>任务 / 计划</th><th>执行目标</th><th>下次执行 / 时区</th><th>最近结果</th><th>操作</th></tr></thead><tbody>${tasks.map(({ task: t, next, last, issue }) => `<tr><td><button class="text-button" data-timer="detail" data-id="${t.id}">${e(t.name)}</button><small><code>${e(t.cron)}</code> · ${t.enabled ? '已启用' : '已暂停'}</small>${issue ? `<small class="timer-danger">${e(issue)}</small>` : ''}</td><td>${e(kinds[t.action.kind])}<small class="timer-path" title="${e(t.action.target)}">${e(t.action.target)}</small></td><td>${this.date(next, t.zone)}<small>${e(t.zone)}</small></td><td>${this.badge(last?.status)}</td><td><div class="timer-row-actions">${button('编辑', 'edit')} ${button(t.enabled ? '暂停' : '启用', 'toggle')} ${button('立即执行', 'run')} ${button('更多', 'more')}</div><span class="timer-id" data-row-id="${t.id}"></span></td></tr>`).join('') || '<tr><td colspan="5" class="timer-empty">没有匹配任务。点击“新建任务”配置第一个计划。</td></tr>'}</tbody></table></div><p class="timer-muted">立即执行使用已保存配置，并真实运行目标程序；暂停只阻止未来调度，不终止当前执行。</p>`;
  }
  private startEdit(task?: Task) {
    this.draft = task
      ? copy(task)
      : freshTask(Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC', this.platform);
    this.baseline = JSON.stringify(this.draft);
    this.step = 0;
    this.mode = task ? 'advanced' : 'simple';
    this.fields = visualFields(this.draft.cron) || visualFields('* * * * *')!;
    this.error = '';
    this.page = 'editor';
    this.render();
    this.queuePreview();
  }
  private field(label: string, name: string, value: string, type = 'text', hint = '') {
    return `<label class="timer-field">${label}<input type="${type}" data-field="${name}" value="${e(value)}">${hint ? `<small>${hint}</small>` : ''}</label>`;
  }
  private editor() {
    return `<div class="timer-editor"><section><nav class="timer-steps" aria-label="任务配置步骤">${['基本信息', '执行计划', '执行动作', '运行策略'].map((n, i) => `<button type="button" data-timer="step" data-step="${i}" aria-current="${i === this.step ? 'step' : 'false'}" class="${i === this.step ? 'selected' : ''}">0${i + 1} ${n}</button>`).join('')}</nav><div class="timer-card">${this.step === 0 ? this.basicForm() : this.step === 1 ? this.cronForm() : this.step === 2 ? this.actionForm() : this.policyForm()}</div><div class="timer-editor-bottom"><span class="timer-muted">${this.dirty() ? '有未保存修改' : '尚未修改'} · 草稿仅在当前标签内保留</span><div class="timer-actions">${this.step > 0 ? button('上一步', 'previous') : ''}${this.step < 3 ? button('下一步 →', 'next', 'primary') : button('保存为暂停', 'save-paused') + button('保存并启用', 'save-enabled', 'primary')}</div></div></section><aside class="timer-preview" aria-live="polite"></aside></div>`;
  }
  private basicForm() {
    const t = this.draft!;
    return `<h2>任务基本信息</h2>${this.field('任务名称', 'name', t.name)}<label class="timer-field">说明<textarea data-field="description" rows="3">${e(t.description)}</textarea></label><p class="timer-muted">任务配置保存在本机，关闭窗口后仍可按计划执行。</p>`;
  }
  private cronForm() {
    const t = this.draft!;
    return `<h2>什么时候执行？</h2><div class="timer-segments">${[
      ['simple', '常用周期'],
      ['fields', '自定义字段'],
      ['advanced', '高级表达式'],
    ]
      .map(
        ([v, n]) =>
          `<button data-timer="cron-mode" data-mode="${v}" class="${v === this.mode ? 'selected' : ''}">${n}</button>`,
      )
      .join('')}</div>${
      this.mode === 'simple'
        ? `<label class="timer-field">重复周期<select data-config="preset">${options(
            [
              ['minutes', '每 N 分钟'],
              ['hourly', '每小时'],
              ['daily', '每天'],
              ['weekly', '每周'],
              ['monthly', '每月'],
            ],
            this.preset,
          )}</select></label>${this.preset === 'minutes' ? `<label class="timer-field">分钟步长<input data-config="period" type="number" min="1" max="59" value="${this.period}"></label>` : this.preset === 'hourly' ? `<label class="timer-field">每小时第几分钟<input data-config="minute" type="number" min="0" max="59" value="${this.minute}"></label>` : `<label class="timer-field">执行时间<input data-config="time" type="time" value="${this.time}"></label>`}${this.preset === 'weekly' ? `<fieldset class="timer-days"><legend>每周执行日期</legend>${['周日', '周一', '周二', '周三', '周四', '周五', '周六'].map((day, i) => `<label><input type="checkbox" data-day="${i}" ${this.days.includes(i) ? 'checked' : ''}>${day}</label>`).join('')}</fieldset><p class="timer-muted">周一至周五不等于法定工作日，不包含节假日与调休。</p>` : ''}${this.preset === 'monthly' ? `<label class="timer-field">日期列表<input data-config="monthdays" value="${e(this.monthdays)}" placeholder="1,15,31"></label><p class="timer-muted">无对应日期时跳过，不自动挪至月底。</p>` : ''}`
        : this.mode === 'fields'
          ? this.fields
              .map(
                (f, i) =>
                  `<div class="timer-cron-field"><label>${['分钟 0–59', '小时 0–23', '日期 1–31', '月份 1–12', '星期 0/7=周日'][i]}<select data-field-mode="${i}">${options(
                    [
                      ['all', '每个值'],
                      ['values', '指定值 / 列表'],
                      ['range', '范围'],
                      ['step', '步长'],
                    ],
                    f.mode,
                  )}</select></label><input aria-label="${['分钟', '小时', '日期', '月份', '星期'][i]}取值" data-field-value="${i}" value="${e(f.value)}" ${f.mode === 'all' ? 'disabled' : ''} placeholder="${f.mode === 'range' ? '1-5' : f.mode === 'step' ? '5' : '1,3,5'}"></div>`,
              )
              .join('')
          : this.field(
              'Cron 表达式',
              'cron',
              t.cron,
              'text',
              '仅数字、*、逗号、范围与步长；不接受 Quartz、秒和年份。',
            )
    }<div class="timer-cron-result"><code data-cron-output>${e(t.cron)}</code>${button('复制', 'copy-cron')}</div>${this.field('IANA 时区', 'zone', t.zone, 'text', '例如 Asia/Shanghai、UTC、America/New_York；保存后不随系统时区变化。')}<p class="timer-muted">预览和实际调度共用 Rust 解析器。日期/星期均受限时按 Unix OR；夏令时缺失跳过、重复时刻只取首次。</p>`;
  }
  private actionForm() {
    const a = this.draft!.action;
    if (a.kind === 'plugin')
      return `<h2>执行原生插件</h2><p>此计划使用插件已保存的配置。配置改变后需更新版本；定时运行引用插件配置的静态口令文件或已有 Git/SVN 认证缓存，不在计划中保存口令。</p>${this.field('插件 ID', 'target', a.target)}${this.field('插件配置版本', 'plugin-revision', String(a.pluginRevision || ''), 'number', '在插件中保存配置后可查看当前版本。')}<p>工作目录、仓库与程序路径在插件中配置，无需脚本解释器。</p>`;
    return `<h2>要执行什么？</h2><label class="timer-field">动作类型<select data-field="kind">${options(
      Object.entries(kinds).filter(([k]) => k !== 'plugin'),
      a.kind,
    )}</select></label><label class="timer-field">${a.kind === 'application' ? '应用绝对路径' : a.kind === 'script' ? '脚本绝对路径' : '程序绝对路径'}<div class="timer-path-picker"><input data-field="target" value="${e(a.target)}" placeholder="请选择本机文件">${button('选择…', 'pick-target')}</div></label>${a.kind === 'script' ? `<label class="timer-field">解释器绝对路径<div class="timer-path-picker"><input data-field="interpreter" value="${e(a.interpreter)}" placeholder="请选择外部解释器，例如 Windows git-bash.exe">${button('选择…', 'pick-interpreter')}</div><small>严格使用所选解释器及继承环境，不自动替换路径。</small></label>${this.platform === 'windows' ? '<p class="timer-muted">Windows 脚本以普通子进程启动。选择 git-bash.exe 时使用无窗口后台命令模式，保留 Git Bash 登录环境，实时显示输出并等待脚本退出；取消与超时尽力停止进程树。</p>' : ''}` : ''}<h3>有序参数</h3><p class="timer-muted">每行是一个独立参数；保留空字符串、空格和引号，不自动展开变量或通配符。</p>${a.args.map((arg, i) => `<div class="timer-argument"><span>${i + 1}</span><input aria-label="参数 ${i + 1}" type="${arg.secret ? 'password' : 'text'}" data-arg="${i}" value="${e(arg.value)}" placeholder="空字符串参数"><label><input type="checkbox" data-secret="${i}" ${arg.secret ? 'checked' : ''}>遮罩</label><button type="button" data-timer="arg-up" data-index="${i}" aria-label="上移参数 ${i + 1}">↑</button><button type="button" data-timer="arg-down" data-index="${i}" aria-label="下移参数 ${i + 1}">↓</button><button type="button" data-timer="arg-remove" data-index="${i}" aria-label="删除参数 ${i + 1}">×</button></div>`).join('')}${button('＋ 添加参数', 'arg-add')}<p class="timer-warning">遮罩只隐藏界面并脱敏执行快照；为支持重启后调度，配置值以明文保存在本机数据库中。请勿在此保存密码或令牌；程序输出不会自动脱敏。</p>${
      a.kind === 'application'
        ? `<label class="timer-field">实例策略<select data-field="instance">${options(
            [
              ['reuse', '请求复用已有实例'],
              ['new', '请求新实例'],
            ],
            a.instance,
          )}</select><small>macOS 使用系统应用启动；其他平台由目标程序决定实例和参数接收行为。</small></label>`
        : `<label class="timer-field">工作目录<div class="timer-path-picker"><input data-field="cwd" value="${e(a.cwd)}" placeholder="留空使用目标所在目录">${button('选择…', 'pick-cwd')}</div></label><h3>环境变量覆盖</h3>${a.env.map((env, i) => `<div class="timer-env"><input aria-label="环境变量名称 ${i + 1}" data-env-key="${i}" value="${e(env.key)}" placeholder="KEY"><input aria-label="环境变量值 ${i + 1}" type="${env.secret ? 'password' : 'text'}" data-env-value="${i}" value="${e(env.value)}"><label><input type="checkbox" data-env-secret="${i}" ${env.secret ? 'checked' : ''}>遮罩</label><button type="button" data-timer="env-remove" data-index="${i}" aria-label="删除环境变量 ${i + 1}">×</button></div>`).join('')}${button('＋ 添加环境变量', 'env-add')}`
    }`;
  }
  private policyForm() {
    const t = this.draft!;
    return `<h2>运行策略与保存</h2><dl class="timer-definition"><dt>同任务重叠</dt><dd>跳过并记录；手动执行同样适用</dd><dt>全局并发</dt><dd>最多 3 个启动/执行；饱和后跳过，不排队</dd><dt>遗漏处理</dt><dd>不补跑；新分钟触发宽限为 30 秒</dd><dt>失败重试</dt><dd>不自动重试，可手动再次执行</dd><dt>输出与历史</dt><dd>单次输出合计最多 10 MiB；保留 30 天，最多 10000 条记录；历史输出总量约 200 MiB</dd></dl>${t.action.kind !== 'application' ? this.field('超时（秒，可留空）', 'timeout', t.timeout === null ? '' : String(t.timeout), 'number', '1–604800 秒；到期停止本轮受管进程组。') : '<p>应用启动仅记录启动请求结果，不监控其内部业务、不提供关闭应用操作。</p>'}<p class="timer-warning">保存并启用后，将按计划真实执行。请核对右侧计划、目标和参数。彻底退出后停止调度；运行任务可选择等待完成或停止后退出。</p>`;
  }
  private queuePreview() {
    clearTimeout(this.debounce);
    this.preview = null;
    this.previewError = '正在校验计划…';
    this.previewSequence++;
    this.renderPreview();
    const token = this.previewSequence;
    this.debounce = setTimeout(async () => {
      if (!this.draft) return;
      try {
        const p = await this.api<Preview>('preview', {
          expression: this.draft.cron,
          zone: this.draft.zone,
        });
        if (token !== this.previewSequence || this.disposed) return;
        this.preview = p;
        this.previewError = '';
      } catch (err) {
        if (token !== this.previewSequence) return;
        this.previewError = String(err);
        this.preview = null;
      }
      this.renderPreview();
    }, 250);
  }
  private renderPreview() {
    const el = this.root.querySelector('.timer-preview');
    if (!el || !this.draft) return;
    const t = this.draft;
    el.innerHTML = `<span class="eyebrow">LIVE PREVIEW</span><h3>执行计划</h3><code>${e(t.cron)}</code><p>${this.preview ? e(this.preview.description) : e(this.previewError)}</p><small>${e(t.zone)} · 分钟粒度</small><h3>未来十次执行</h3><ol>${this.preview?.times.map((at) => `<li>${this.date(at, t.zone)}</li>`).join('') || '<li>计划校验通过后显示</li>'}</ol><h3>执行动作</h3><p>${e(kinds[t.action.kind])}</p><code class="timer-preview-path">${e(t.action.target || '尚未选择目标')}</code>${t.action.kind === 'script' ? `<small>解释器 ${e(t.action.interpreter)}</small>` : ''}<ol>${t.action.args.map((arg) => `<li><code>${arg.secret ? '••••••' : arg.value === '' ? '〈空参数〉' : e(arg.value)}</code></li>`).join('') || '<li>无参数</li>'}</ol><small>这里只校验与预览，不执行动作。</small>`;
  }
  private detail() {
    const item = this.data.tasks.find((i) => i.task.id === this.detailId);
    if (!item) return '<p>任务已被删除。</p>';
    const t = item.task;
    return `<section class="timer-card"><h2>${e(t.name)}</h2><p>${e(t.description)}</p><dl class="timer-definition"><dt>配置版本</dt><dd>${t.revision}</dd><dt>计划状态</dt><dd>${t.enabled ? '已启用' : '已暂停'}</dd><dt>Cron / 时区</dt><dd><code>${e(t.cron)}</code> · ${e(t.zone)}</dd><dt>动作目标</dt><dd>${e(kinds[t.action.kind])} · ${e(t.action.target)}</dd><dt>下次执行</dt><dd>${this.date(item.next, t.zone)}</dd><dt>最近结果</dt><dd>${this.badge(item.last?.status)}</dd></dl>${item.issue ? `<p class="timer-error">${e(item.issue)}</p>` : ''}<div class="timer-actions">${button('编辑', 'edit')}${button('立即执行', 'run', 'primary')}${button('执行记录', 'task-history')}${button('复制为暂停任务', 'duplicate')}${button('删除任务', 'delete', 'danger')}</div></section>`;
  }
  private historyPage() {
    return `<div class="timer-toolbar"><label>任务筛选<select data-filter="history">${options([['', '全部任务'], ...this.data.tasks.map((i) => [i.task.id, i.task.name] as [string, string])], this.detailId)}</select></label>${button('刷新', 'refresh-history')}</div><div class="timer-table-scroll"><table class="timer-table"><thead><tr><th>任务 / 配置版本</th><th>时间 / 来源</th><th>结果 / 耗时</th><th>操作</th></tr></thead><tbody>${this.history.map((r) => `<tr><td>${e(r.snapshot.name)}<small>v${r.revision}</small></td><td>${this.date(r.startedAt)}<small>${r.source === 'manual' ? '手动执行' : '计划触发'} · 计划 ${this.date(r.scheduledAt, r.snapshot.zone)}</small></td><td>${this.badge(r.status)}<small>${r.finishedAt ? `${((+new Date(r.finishedAt) - +new Date(r.startedAt)) / 1000).toFixed(1)} 秒` : '运行中'}</small></td><td><button class="button" data-timer="log" data-id="${r.id}">日志与快照</button>${r.status === 'running' && r.snapshot.action.kind !== 'application' ? `<button class="button danger" data-timer="cancel-run" data-id="${r.id}">停止本轮</button>` : ''}</td></tr>`).join('') || '<tr><td colspan="4" class="timer-empty">暂无执行记录。</td></tr>'}</tbody></table></div><div class="timer-actions">${this.offset > 0 ? button('上一页', 'history-prev') : ''}${this.history.length === 100 ? button('下一页', 'history-next') : ''}<span>每页最多 100 条 · 时间按本机时区显示</span></div>`;
  }
  private renderLog() {
    if (!this.log) return;
    const host = this.root.querySelector('.timer-dialog-host');
    if (!host) return;
    let d = host.querySelector('dialog');
    const fresh = !d;
    if (!d) {
      host.innerHTML =
        '<dialog class="timer-log" aria-label="执行日志"><div class="timer-log-content"></div></dialog>';
      d = host.querySelector('dialog')!;
      d.addEventListener('close', () => {
        this.log = null;
        d?.remove();
      });
    }
    const r = this.log;
    if (!fresh) {
      const pre = d.querySelectorAll('pre');
      const scroll = [...pre].map((p) => ({
        top: p.scrollTop,
        follow: p.scrollHeight - p.clientHeight - p.scrollTop < 32,
      }));
      d.querySelector('[data-run-message]')!.textContent =
        `${r.message} · 退出码 ${r.exitCode === null ? '—' : r.exitCode}`;
      d.querySelector('[data-run-status]')!.textContent =
        `${r.snapshot.name} · ${statuses[r.status]}`;
      pre[0].textContent = r.stdout || '无输出';
      pre[1].textContent = r.stderr || '无输出';
      pre.forEach((p, i) => {
        p.scrollTop = scroll[i].follow ? p.scrollHeight : scroll[i].top;
      });
      const note = d.querySelector<HTMLElement>('[data-truncated]')!;
      note.hidden = !r.truncated;
      return;
    }
    d.querySelector('.timer-log-content')!.innerHTML =
      `<div class="timer-header"><h2 data-run-status>${e(r.snapshot.name)} · ${e(statuses[r.status])}</h2>${button('关闭', 'close-log')}</div><p data-run-message>${e(r.message)} · 退出码 ${r.exitCode === null ? '—' : r.exitCode}</p><p data-truncated class="timer-warning" ${r.truncated ? '' : 'hidden'}>输出已截断或因容量清理，未显示的内容不可恢复。</p><p>运行中每 1.5 秒刷新输出；脚本内程序的缓冲可能延迟日志。</p><h3>标准输出</h3><pre>${e(r.stdout || '无输出')}</pre><h3>错误输出</h3><pre>${e(r.stderr || '无输出')}</pre><details><summary>执行配置快照（敏感项已脱敏）</summary><pre>${e(JSON.stringify(r.snapshot, null, 2))}</pre></details>`;
    if (fresh) {
      d.showModal();
      if (r.status === 'running')
        [...d.querySelectorAll('pre')].slice(0, 2).forEach((p) => {
          p.scrollTop = p.scrollHeight;
        });
      (d.querySelector('button') as HTMLElement)?.focus();
    }
  }
  private confirm(message: string): Promise<boolean> {
    return this.api<boolean>('confirm', { message });
  }
  private async leave() {
    return !this.dirty() || (await this.confirm('有未保存修改。是否放弃本次修改并离开？'));
  }
  private async refreshHistory() {
    this.history = await this.api<Run[]>('history', {
      taskId: this.detailId || null,
      offset: this.offset,
    });
    this.render();
  }
  private click = (event: MouseEvent) => {
    const b = (event.target as Element).closest<HTMLButtonElement>('[data-timer]');
    if (!b) return;
    event.stopPropagation();
    if (this.busy) return;
    void this.act(b).catch((err) => {
      this.error = String(err);
      this.render();
    });
  };
  private async act(b: HTMLButtonElement) {
    const action = b.dataset.timer!;
    const id =
      b.dataset.id ||
      b.closest('tr')?.querySelector<HTMLElement>('[data-row-id]')?.dataset.rowId ||
      this.detailId;
    const task = this.data.tasks.find((i) => i.task.id === id)?.task;
    this.error = '';
    if (action === 'new') {
      if (await this.leave()) this.startEdit();
      return;
    }
    if (action === 'list') {
      if (await this.leave()) {
        this.draft = null;
        this.page = 'list';
        await this.load();
      }
      return;
    }
    if (action === 'detail') {
      this.detailId = id!;
      this.page = 'detail';
      this.render();
      return;
    }
    if (action === 'edit' && task) {
      this.startEdit(task);
      return;
    }
    if (action === 'history' || action === 'task-history') {
      if (!(await this.leave())) return;
      this.draft = null;
      this.detailId = action === 'history' ? '' : id!;
      this.page = 'history';
      this.offset = 0;
      await this.refreshHistory();
      return;
    }
    if (action === 'more' && task) {
      this.detailId = task.id;
      this.page = 'detail';
      this.render();
      return;
    }
    if (action === 'duplicate' && task) {
      const c = copy(task);
      c.id = '';
      c.revision = 0;
      c.name += '（副本）';
      c.enabled = false;
      await this.api('save', { task: c });
      this.page = 'list';
      await this.load();
      return;
    }
    if (action === 'delete' && task) {
      if (await this.confirm(`删除“${task.name}”？计划会停用，历史保留，当前运行不会停止。`)) {
        await this.api('delete', { id });
        this.page = 'list';
        await this.load();
      }
      return;
    }
    if (action === 'toggle' && task) {
      if (
        task.enabled ||
        (await this.confirm(`启用“${task.name}”？将按保存的计划真实执行 ${task.action.target}`))
      ) {
        await this.api('toggle', { id, enabled: !task.enabled });
        await this.load();
      }
      return;
    }
    if (action === 'run' && task) {
      if (
        !(await this.confirm(
          `立即真实执行“${task.name}”？使用已保存的 v${task.revision} 配置，目标：${task.action.target}`,
        ))
      )
        return;
      this.busy = true;
      try {
        const runId = await this.api<string>('run', { id });
        this.log = await this.api<Run>('run_detail', { id: runId });
        await this.poll();
        this.render();
      } finally {
        this.busy = false;
      }
      return;
    }
    if (action === 'log') {
      this.log = await this.api<Run>('run_detail', { id });
      this.renderLog();
      return;
    }
    if (action === 'close-log') {
      this.log = null;
      this.root.querySelector<HTMLDialogElement>('dialog')?.close();
      return;
    }
    if (action === 'cancel-run') {
      await this.api('cancel', { id });
      await this.refreshHistory();
      return;
    }
    if (action.startsWith('history-')) {
      this.offset = Math.max(0, this.offset + (action === 'history-next' ? 100 : -100));
      await this.refreshHistory();
      return;
    }
    if (action === 'refresh-history') {
      await this.refreshHistory();
      return;
    }
    if (action === 'step' || action === 'next' || action === 'previous') {
      this.step =
        action === 'step' ? Number(b.dataset.step) : this.step + (action === 'next' ? 1 : -1);
      this.render();
      return;
    }
    if (action === 'cron-mode') {
      const next = b.dataset.mode!;
      if (next === 'fields') {
        const fields = visualFields(this.draft!.cron);
        if (!fields) {
          this.error = '组合语法不能无损回填，已保留高级表达式。';
          this.render();
          return;
        }
        this.fields = fields;
      }
      if (next === 'simple' && this.mode !== 'simple') {
        if (!(await this.confirm('常用周期将用当前周期选项重新生成表达式，是否继续？'))) return;
        this.generate();
      }
      this.mode = next;
      this.render();
      this.queuePreview();
      return;
    }
    if (action === 'copy-cron') {
      await navigator.clipboard.writeText(this.draft!.cron);
      return;
    }
    if (action.startsWith('save-')) {
      this.busy = true;
      try {
        const t = copy(this.draft!);
        t.enabled = action === 'save-enabled';
        await this.api('save', { task: t });
        this.draft = null;
        this.page = 'list';
        await this.load();
      } finally {
        this.busy = false;
      }
      return;
    }
    const a = this.draft?.action;
    if (!a) return;
    if (action.startsWith('pick-')) {
      const field = action.slice(5);
      const kind =
        field === 'cwd'
          ? 'folder'
          : field === 'interpreter'
            ? 'interpreter'
            : a.kind === 'executable'
              ? 'target'
              : a.kind;
      const path = await this.api<string | null>('pick', { kind });
      if (path) {
        if (field === 'target') a.target = path;
        else if (field === 'cwd') a.cwd = path;
        else a.interpreter = path;
        this.render();
        this.renderPreview();
      }
      return;
    }
    const index = Number(b.dataset.index);
    if (action === 'arg-add') a.args.push({ value: '', secret: false });
    if (action === 'arg-remove') a.args.splice(index, 1);
    if (action === 'arg-up' && index > 0)
      [a.args[index - 1], a.args[index]] = [a.args[index], a.args[index - 1]];
    if (action === 'arg-down' && index < a.args.length - 1)
      [a.args[index + 1], a.args[index]] = [a.args[index], a.args[index + 1]];
    if (action === 'env-add') a.env.push({ key: '', value: '', secret: false });
    if (action === 'env-remove') a.env.splice(index, 1);
    this.render();
  }
  private generate() {
    this.draft!.cron = generatedCron(
      this.preset,
      this.period,
      this.minute,
      this.time,
      this.days,
      this.monthdays,
    );
  }
  private input = (event: Event) => {
    const el = event.target as HTMLInputElement;
    const d = el.dataset;
    if (!this.draft) {
      if (d.filter === 'search') {
        this.search = el.value;
        const start = el.selectionStart;
        this.render();
        const input = this.root.querySelector<HTMLInputElement>('[data-filter="search"]')!;
        input.focus();
        input.setSelectionRange(start, start);
      }
      return;
    }
    const t = this.draft,
      a = t.action;
    if (d.field) {
      const v = el.value;
      switch (d.field) {
        case 'name':
          t.name = v;
          break;
        case 'description':
          t.description = v;
          break;
        case 'cron':
          t.cron = v;
          break;
        case 'zone':
          t.zone = v;
          break;
        case 'target':
          a.target = v;
          break;
        case 'interpreter':
          a.interpreter = v;
          break;
        case 'plugin-revision':
          a.pluginRevision = Number(v);
          break;
        case 'cwd':
          a.cwd = v;
          break;
        case 'instance':
          a.instance = v;
          break;
        case 'timeout':
          t.timeout = v === '' ? null : Number(v);
          break;
      }
    }
    if (d.arg !== undefined) a.args[Number(d.arg)].value = el.value;
    if (d.envKey !== undefined) a.env[Number(d.envKey)].key = el.value;
    if (d.envValue !== undefined) a.env[Number(d.envValue)].value = el.value;
    if (d.config) {
      if (d.config === 'period') this.period = Number(el.value);
      if (d.config === 'minute') this.minute = Number(el.value);
      if (d.config === 'time') this.time = el.value;
      if (d.config === 'monthdays') this.monthdays = el.value;
      this.generate();
    }
    if (d.fieldValue !== undefined) {
      this.fields[Number(d.fieldValue)].value = el.value;
      this.generateFields();
    }
    if (d.field === 'cron' || d.field === 'zone' || d.config || d.fieldValue !== undefined) {
      const out = this.root.querySelector('[data-cron-output]');
      if (out) out.textContent = t.cron;
      this.queuePreview();
    } else this.renderPreview();
  };
  private generateFields() {
    this.draft!.cron = this.fields
      .map((f) => (f.mode === 'all' ? '*' : f.mode === 'step' ? `*/${f.value}` : f.value))
      .join(' ');
  }
  private change = (event: Event) => {
    const el = event.target as HTMLInputElement;
    const d = el.dataset;
    if (d.filter === 'kind' || d.filter === 'status') {
      if (d.filter === 'kind') this.kind = el.value;
      else this.status = el.value;
      this.render();
      return;
    }
    if (d.filter === 'history') {
      this.detailId = el.value;
      this.offset = 0;
      void this.refreshHistory().catch((err) => {
        this.error = String(err);
        this.render();
      });
      return;
    }
    if (!this.draft) return;
    const a = this.draft.action;
    if (d.field === 'kind') {
      a.kind = el.value;
      a.target = '';
      a.args = [];
      a.env = [];
      a.cwd = '';
      this.draft.timeout = null;
      a.instance = 'reuse';
      this.render();
      return;
    }
    if (d.config === 'preset') {
      this.preset = el.value;
      this.generate();
      this.render();
      this.queuePreview();
      return;
    }
    if (d.day !== undefined) {
      const day = Number(d.day);
      this.days = el.checked ? [...this.days, day].sort() : this.days.filter((n) => n !== day);
      this.generate();
      this.render();
      this.queuePreview();
      return;
    }
    if (d.fieldMode !== undefined) {
      const i = Number(d.fieldMode);
      this.fields[i] = {
        mode: el.value,
        value:
          el.value === 'range'
            ? '1-5'
            : el.value === 'step'
              ? '5'
              : el.value === 'values'
                ? '1'
                : '',
      };
      this.generateFields();
      this.render();
      this.queuePreview();
      return;
    }
    if (d.secret !== undefined) a.args[Number(d.secret)].secret = el.checked;
    if (d.envSecret !== undefined) a.env[Number(d.envSecret)].secret = el.checked;
    if (d.secret !== undefined || d.envSecret !== undefined) this.render();
  };
}
