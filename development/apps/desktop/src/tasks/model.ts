export interface Argument {
  value: string;
  secret: boolean;
}
export interface Environment {
  key: string;
  value: string;
  secret: boolean;
}
export interface Task {
  id: string;
  revision: number;
  name: string;
  description: string;
  enabled: boolean;
  cron: string;
  zone: string;
  action: {
    kind: string;
    target: string;
    interpreter: string;
    args: Argument[];
    env: Environment[];
    cwd: string;
    instance: string;
    pluginRevision?: number;
  };
  timeout: number | null;
}
export interface Run {
  id: string;
  taskId: string;
  revision: number;
  source: string;
  scheduledAt: string;
  startedAt: string;
  finishedAt: string | null;
  status: string;
  exitCode: number | null;
  message: string;
  stdout: string;
  stderr: string;
  truncated: boolean;
  snapshot: Task;
}
export interface Item {
  task: Task;
  next: string | null;
  last: Run | null;
  issue: string | null;
}
export interface Overview {
  tasks: Item[];
  running: number;
  concurrency: number;
  stopping: boolean;
  notice: string;
}
export interface Preview {
  expression: string;
  description: string;
  times: string[];
  fields: number[][];
}
export function freshTask(zone: string, platform: string): Task {
  return {
    id: '',
    revision: 0,
    name: '',
    description: '',
    enabled: false,
    cron: '0 9 * * 1-5',
    zone,
    action: {
      kind: 'script',
      target: '',
      interpreter: platform === 'windows' ? '' : '/bin/sh',
      args: [],
      env: [],
      cwd: '',
      instance: 'reuse',
    },
    timeout: null,
  };
}
export function visualFields(expression: string): { mode: string; value: string }[] | null {
  const parts = expression.trim().split(/\s+/);
  if (parts.length !== 5) return null;
  const out = [];
  for (const raw of parts) {
    if (raw === '*') out.push({ mode: 'all', value: '' });
    else if (/^\d+(,\d+)*$/.test(raw)) out.push({ mode: 'values', value: raw });
    else if (/^\d+-\d+$/.test(raw)) out.push({ mode: 'range', value: raw });
    else if (/^\*\/\d+$/.test(raw)) out.push({ mode: 'step', value: raw.slice(2) });
    else return null;
  }
  return out;
}
export function generatedCron(
  preset: string,
  period: number,
  minute: number,
  time: string,
  days: number[],
  monthdays: string,
): string {
  const [h, m] = time.split(':').map(Number);
  if (preset === 'minutes') return `*/${period} * * * *`;
  if (preset === 'hourly') return `${minute} * * * *`;
  if (preset === 'weekly') return `${m} ${h} * * ${days.join(',')}`;
  if (preset === 'monthly') return `${m} ${h} ${monthdays} * *`;
  return `${m} ${h} * * *`;
}
export const statuses: Record<string, string> = {
  running: '执行中',
  succeeded: '执行成功',
  failed: '执行失败',
  launched: '启动请求成功',
  skipped: '已跳过',
  cancelled: '已取消',
  timed_out: '已超时',
  interrupted: '中断 / 结果未知',
};
