/** Tool metadata is the registration boundary. Business implementations come later. */
export const categories = ['格式与校验', '时间与任务'] as const;
export type Category = typeof categories[number];
export const tools = [
  { id: 'json', name: 'JSON 格式化', symbol: '{ }', category: '格式与校验', color: 'green', description: '让结构清晰，让数据易读', tags: '格式化 · 压缩 · 校验', keywords: 'json format validate' },
  { id: 'xml', name: 'XML 格式化', symbol: '</>', category: '格式与校验', color: 'orange', description: '整理层级，检查文档结构', tags: '格式化 · 语法校验', keywords: 'xml format validate' },
  { id: 'timestamp', name: '时间戳转换', symbol: '◷', category: '时间与任务', color: 'blue', description: '在时间戳与日期之间自如转换', tags: '秒 / 毫秒 · 时区', keywords: 'timestamp date time unix' },
  { id: 'cron', name: 'cron 表达式', symbol: '✳', category: '时间与任务', color: 'purple', description: '读懂每一次计划执行', tags: '表达式 · 执行预览', keywords: 'cron schedule' },
  { id: 'tasks', name: '定时任务', symbol: '☷', category: '时间与任务', color: 'rose', description: '把重复的小事交给计划', tags: 'Cron · 脚本 · 程序 · 执行记录', keywords: 'task timer reminder schedule' },
] as const;
export type Tool = typeof tools[number];
export type ToolId = Tool['id'];
export type TabId = 'home' | ToolId;
export const findTool = (id: string): Tool | undefined => tools.find(tool => tool.id === id);
export const isToolId = (id: unknown): id is ToolId => typeof id === 'string' && !!findTool(id);
export const isTabId = (id: unknown): id is TabId => id === 'home' || isToolId(id);
export function matchesSearch(tool: Tool, query: string): boolean {
  const needle = query.trim().toLocaleLowerCase();
  return `${tool.name} ${tool.category} ${tool.keywords}`.toLocaleLowerCase().includes(needle);
}
