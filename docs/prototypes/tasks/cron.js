/* Prototype-only unix5 parser/preview; production scheduling will use the Rust service.
 * Fields: minute hour day month weekday. Numeric *, lists, ranges and steps.
 * Local DST gaps are skipped and repeated wall times use the first occurrence.
 */
(function (scope) {
  const limits = [[0,59],[0,23],[1,31],[1,12],[0,7]];
  const names = ['分钟','小时','日期','月份','星期'];
  function field(raw, index) {
    const [min,max] = limits[index], values = new Set();
    if (!raw || !/^[\d*,/\-]+$/.test(raw)) throw Error(`${names[index]}字段仅支持数字、*、逗号、范围和步长`);
    for (const token of raw.split(',')) {
      const match = /^(\*|\d+(?:-\d+)?)(?:\/(\d+))?$/.exec(token);
      if (!match) throw Error(`${names[index]}字段格式无效`);
      const step = match[2] ? Number(match[2]) : 1;
      if (step < 1 || step > max - min + 1) throw Error(`${names[index]}步长应在 1–${max-min+1} 之间`);
      let start, end;
      if (match[1] === '*') { start = min; end = max; }
      else { const range = match[1].split('-').map(Number); start = range[0]; end = range[1] ?? (match[2] ? max : start); }
      if (start < min || end > max || start > end) throw Error(`${names[index]}应在 ${min}–${max} 之间，且范围起点不能大于终点`);
      for (let value = start; value <= end; value += step) values.add(index === 4 && value === 7 ? 0 : value);
    }
    return { raw, values: [...values].sort((a,b)=>a-b), wildcard: raw.split(',').some(token => token.startsWith('*')) };
  }
  function parse(expression) {
    const parts = expression.trim().split(/\s+/);
    if (parts.length !== 5) throw Error('当前设计使用 Unix 五字段：分钟 小时 日期 月份 星期，不包含秒或年份');
    return { expression: parts.join(' '), fields: parts.map(field) };
  }
  function describe(expression) {
    const parsed = parse(expression), [m,h,d,mo,w] = parsed.fields;
    const weekday = ['周日','周一','周二','周三','周四','周五','周六'];
    const time = h.values.length === 1 && m.values.length === 1 ? `${String(h.values[0]).padStart(2,'0')}:${String(m.values[0]).padStart(2,'0')}` : null;
    if (d.raw === '*' && mo.raw === '*' && w.raw === '*') {
      if (h.raw === '*' && /^\*\/\d+$/.test(m.raw)) return `每小时内，每 ${m.raw.slice(2)} 分钟执行`;
      if (h.raw === '*' && m.raw === '*') return '每分钟执行一次';
      if (h.raw === '*' && m.values.length === 1) return `每小时第 ${m.values[0]} 分钟执行`;
      if (time) return `每天 ${time} 执行`;
    }
    if (time && mo.raw === '*' && d.raw === '*' && w.raw !== '*') return `${w.values.map(value=>weekday[value]).join('、')} ${time} 执行`;
    if (time && mo.raw === '*' && d.raw !== '*' && w.raw === '*') return `每月 ${d.values.join('、')} 日 ${time} 执行（无对应日期则跳过）`;
    const combined = !d.wildcard && !w.wildcard ? '；日期与星期按「或」匹配' : '';
    return `分钟 ${m.raw} · 小时 ${h.raw} · 日期 ${d.raw} · 月份 ${mo.raw} · 星期 ${w.raw}${combined}`;
  }
  function visualFields(expression) {
    return parse(expression).fields.map((item, index) => {
      const raw = item.raw;
      if (raw === '*') return { mode:'all', value:'' };
      if (/^\d+(?:,\d+)*$/.test(raw)) return { mode:'values', value:raw };
      if (/^\d+-\d+$/.test(raw)) return { mode:'range', value:raw };
      if (/^\*\/\d+$/.test(raw)) return { mode:'step', value:raw.slice(2) };
      throw Error(`${names[index]}含组合语法，不能无损回填；已保留高级表达式`);
    });
  }
  const formatters = new Map();
  function formatter(zone) {
    if (!formatters.has(zone)) formatters.set(zone, new Intl.DateTimeFormat('en-CA',{timeZone:zone,year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hourCycle:'h23'}));
    return formatters.get(zone);
  }
  function dateParts(date, zone) {
    return Object.fromEntries(formatter(zone).formatToParts(date).filter(part=>part.type!=='literal').map(part=>[part.type, Number(part.value)]));
  }
  function matchingDay(parsed, date) {
    const [, ,dom,month,dow] = parsed.fields;
    if (!month.values.includes(date.getUTCMonth()+1)) return false;
    const a = dom.values.includes(date.getUTCDate()), b = dow.values.includes(date.getUTCDay());
    return dom.wildcard || dow.wildcard ? a && b : a || b;
  }
  function offsetsForDay(date, zone) {
    const noon = Date.UTC(date.getUTCFullYear(),date.getUTCMonth(),date.getUTCDate(),12);
    return [...new Set([-36,0,36].map(hours=> {
      const sample = noon + hours*3600000, p = dateParts(new Date(sample),zone);
      return Date.UTC(p.year,p.month-1,p.day,p.hour,p.minute,p.second) - sample;
    }))];
  }
  async function next(expression, zone, count=10, now=new Date(), cancelled=()=>false) {
    const parsed=parse(expression), local=dateParts(now,zone), start=Date.UTC(local.year,local.month-1,local.day), found=[];
    for(let day=0;day<366*50 && found.length<count;day++) {
      if(cancelled()) return [];
      if(day && day%250===0) await new Promise(resolve=>setTimeout(resolve,0));
      const date=new Date(start+day*86400000);
      if(!matchingDay(parsed,date)) continue;
      const offsets=offsetsForDay(date,zone), candidates=[];
      for(const hour of parsed.fields[1].values) for(const minute of parsed.fields[0].values) {
        const wall=Date.UTC(date.getUTCFullYear(),date.getUTCMonth(),date.getUTCDate(),hour,minute);
        const matches=offsets.map(offset=>new Date(wall-offset)).filter(candidate=>{
          const p=dateParts(candidate,zone);
          return p.year===date.getUTCFullYear()&&p.month===date.getUTCMonth()+1&&p.day===date.getUTCDate()&&p.hour===hour&&p.minute===minute;
        }).sort((a,b)=>a-b);
        // First occurrence only. Never turn a missing time into the next hour.
        if(matches[0] && matches[0]>now) candidates.push(matches[0]);
        // For each day order by local slots; with first-fold policy they also order by UTC.
        if(candidates.length>=count-found.length) break;
      }
      found.push(...candidates.sort((a,b)=>a-b).slice(0,count-found.length));
    }
    return found;
  }
  function equivalent(a,b) {
    try {const left=parse(a),right=parse(b);return left.fields.every((item,index)=>item.wildcard===right.fields[index].wildcard&&JSON.stringify(item.values)===JSON.stringify(right.fields[index].values));} catch {return false;}
  }
  const api={parse,describe,visualFields,next,equivalent,limits,names};
  if(typeof module!=='undefined'&&module.exports) module.exports=api;
  else scope.HandyCron=api;
})(globalThis);
