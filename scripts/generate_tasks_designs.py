"""Generate static design boards corresponding to the interactive tasks prototype."""
from pathlib import Path
from html import escape
import argparse
import subprocess
import tempfile
import struct
import zlib

OUT = Path(__file__).resolve().parents[1] / 'docs/prototypes/tasks/designs'
INK = '#526b43'
MUTED = '#96a787'
LINE = '#e2e9d9'


def crop_top_png(source, destination, height):
    """Keep top scanlines of a non-interlaced PNG without resampling the design."""
    blob=source.read_bytes()
    assert blob[:8] == b'\x89PNG\r\n\x1a\n'
    offset=8
    chunks=[]
    while offset<len(blob):
        length=struct.unpack('>I',blob[offset:offset+4])[0]
        kind=blob[offset+4:offset+8]
        data=blob[offset+8:offset+8+length]
        chunks.append((kind,data))
        offset+=12+length
    header=next(data for kind,data in chunks if kind==b'IHDR')
    width,old_height,depth,color,compression,filter_method,interlace=struct.unpack('>IIBBBBB',header)
    assert interlace==0 and 0<height<=old_height
    channels={0:1,2:3,3:1,4:2,6:4}[color]
    stride=1+(width*depth*channels+7)//8
    raw=zlib.decompress(b''.join(data for kind,data in chunks if kind==b'IDAT'))[:stride*height]
    def chunk(kind,data):
        return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    result=blob[:8]+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,depth,color,compression,filter_method,interlace))
    result+=b''.join(chunk(kind,data) for kind,data in chunks if kind not in {b'IHDR',b'IDAT',b'IEND'})
    result+=chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
    destination.write_bytes(result)


def rect(x, y, w, h, fill='#fff', stroke=LINE, r=6):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}"/>'


def text(x, y, value, size=12, color=INK, weight=400, mono=False):
    font = 'SFMono-Regular,Menlo,monospace' if mono else 'PingFang SC,Microsoft YaHei,sans-serif'
    return f'<text x="{x}" y="{y}" fill="{color}" font-family="{font}" font-size="{size}" font-weight="{weight}">{escape(str(value))}</text>'


def line(x1, y1, x2, y2):
    return f'<path d="M{x1} {y1}H{x2}" stroke="{LINE}"/>' if y1 == y2 else f'<path d="M{x1} {y1}L{x2} {y2}" stroke="{LINE}"/>'


def button(x, y, w, label, primary=False):
    return rect(x, y, w, 34, '#4d7951' if primary else '#fff', '#4d7951' if primary else LINE, 5) + text(x+13, y+22, label, 11, '#fff' if primary else '#80986b')


def field(x, y, w, label, value):
    return text(x, y, label, 11, '#8ba077') + rect(x, y+12, w, 37, '#fcfdfb') + text(x+12, y+36, value, 12, '#779461')


def badge(x, y, value, color='#86a46c', fill='#edf5e5', width=64):
    return rect(x, y-14, width, 23, fill, fill, 4) + text(x+9, y+1, value, 9, color)


def frame(title, subtitle, board):
    value = f'<svg xmlns="http://www.w3.org/2000/svg" width="1380" height="920" viewBox="0 0 1380 920"><title>{escape(title)} · 静态设计图</title>'
    value += rect(0, 0, 1380, 920, '#f9fbf7', '#e0e7d7', 12)
    value += rect(0, 0, 1380, 58, '#fff', LINE, 0)
    value += rect(23, 13, 32, 32, '#315f49', '#315f49', 8)
    value += '<path d="M31 26L39 20L47 26M35 31H43M39 31V39M33 40H45" stroke="#f5ebd3" stroke-width="2.6" fill="none" stroke-linecap="round"/>'
    value += text(67, 36, '小工坊', 17, '#3e4b35', 600) + text(137, 36, 'HandyKit', 13, '#88967e')
    value += badge(210, 32, 'TASKS / PROTOTYPE', '#9aad89', '#f3f6ef', 122) + text(1190, 34, '本地工作空间  /  设计评审', 10, '#9bac8c')
    value += rect(0, 58, 205, 833, '#f1f5ec', LINE, 0)
    value += text(25, 96, 'WORKSPACE', 10, '#9aab8a', mono=True) + text(27, 140, '⌂   工作台首页', 12, '#809471')
    value += text(27, 198, '格式与校验', 10, '#a2b293') + text(30, 236, '{ }   JSON 格式化', 12, '#839976') + text(30, 276, '</>   XML 格式化', 12, '#839976')
    value += text(27, 334, '时间与任务', 10, '#a2b293') + text(30, 373, '◷   时间戳转换', 12, '#839976') + text(30, 413, '✳   cron 表达式', 12, '#839976')
    value += rect(14, 434, 176, 39, '#e3eddb', '#e3eddb', 5) + text(29, 459, '☷   定时任务', 12, '#638451', 500)
    value += text(27, 788, '把重复的小事，交给计划。', 11, '#8fa67b') + text(27, 813, '窗口收起，计划继续。', 10, '#a6b798')
    value += rect(205, 58, 1175, 42, '#edf2e8', LINE, 0) + text(227, 84, '⌂  首页', 11, '#a4b297') + text(350, 84, '{ }  JSON 格式化', 11, '#a4b297')
    value += rect(495, 58, 160, 42, '#f9fbf7', LINE, 0) + line(495, 59, 655, 59) + text(517, 84, '☷  定时任务      ×', 11, '#7c9768')
    value += rect(205, 100, 1175, 30, '#f7f3e9', '#ede8d9', 0) + text(231, 120, '交互原型  ·  示例数据与模拟执行，不访问本机文件，不运行脚本或程序。', 10, '#b19b73')
    value += text(233, 159, '工具箱   /   时间与任务   /   定时任务', 10, '#a4b595')
    value += text(235, 208, title, 25, '#425437', 600) + text(236, 234, subtitle, 11, '#9aad8c')
    value += rect(0, 891, 1380, 29, '#f0f5e9', LINE, 0) + text(16, 910, f'●  定时任务     │     {board} · 静态设计图', 9, '#9caf8b') + text(1128, 910, 'HANDYKIT  /  DESIGN PREVIEW', 9, '#afbc9f', mono=True)
    return value


def list_board():
    s = frame('定时任务', '让计划准时发生，让执行有迹可循。', '01')
    s += button(1112, 185, 94, '运行设置') + button(1217, 185, 124, '＋ 新建任务', True)
    for index, (label, number, color) in enumerate([('全部任务', '06', '#789562'), ('计划已启用', '04', '#789562'), ('模拟执行中', '01', '#8da6b8'), ('最近执行失败', '01', '#b99a72')]):
        x = 234+index*279
        s += rect(x, 265, 265, 88) + text(x+18, 291, label, 10, '#a2b391') + text(x+18, 328, number, 26, color, mono=True)
    s += text(235, 392, '任务列表  06', 13, '#6f8e55', 500) + text(351, 392, '执行记录  06', 12, '#a7b698') + line(234, 407, 1341, 407) + line(234, 406, 315, 406)
    s += field(234, 430, 240, '', '⌕   搜索任务名称、目标文件…') + field(489, 430, 140, '', '全部计划状态   ⌄') + field(643, 430, 140, '', '全部动作类型   ⌄') + text(1241, 466, '显示 6 / 6 项任务', 10, '#a8b997')
    s += rect(234, 492, 1107, 332) + rect(234, 492, 1107, 37, '#f1f6eb', LINE, 0)
    columns = [(250, '任务名称'), (435, '执行计划'), (600, '执行动作'), (780, '计划状态'), (888, '下次执行'), (1030, '最近结果'), (1177, '操作')]
    for x, label in columns: s += text(x, 516, label, 10, '#9eaf8c')
    rows = [('每日项目备份', '0 18 * * 1-5', '运行脚本', 'daily-backup.sh', '已启用', '11/23 18:00', '执行成功'), ('打开开发工作区', '0 9 * * 1-5', '打开应用', 'Visual Studio Code.app', '已启用', '11/23 09:00', '启动请求成功'), ('生成每日构建报告', '30 17 * * 1-5', '运行程序', 'report-generator', '已启用', '11/23 17:30', '执行失败'), ('更新项目索引', '*/15 * * * *', '运行脚本', 'update-index.sh', '已启用', '11/20 12:15', '执行中'), ('每周清理临时文件', '0 20 * * 0', '运行脚本', 'cleanup.sh', '已暂停', '—', '执行成功')]
    for i, (name, cron, kind, target, enabled, next_run, result) in enumerate(rows):
        y=562+i*55
        s += text(250, y, name, 12, '#718c59', 500) + text(250, y+17, '本地开发任务 · 示例', 9, '#b2c0a2')
        s += text(435, y, cron, 10, '#90a779', mono=True) + text(600, y, kind, 10, '#9aad87') + text(600, y+17, target, 9, '#adbd9c', mono=True)
        s += badge(780, y, enabled, '#8aa66f' if enabled=='已启用' else '#a6b698') + text(888, y, next_run, 10, '#91a97a', mono=True)
        s += badge(1030, y, result, '#b38f70' if result=='执行失败' else '#8fa77a', '#f7eee3' if result=='执行失败' else '#eef5e6', 94)
        s += text(1177, y, '编辑   立即执行   ···', 10, '#97ad83') + line(246, y+28, 1328, y+28)
    s += text(235, 854, '◇  窗口关闭，计划继续。彻底退出后停止调度；所有动作均为原型模拟。', 10, '#a0b38b')
    return s+'</svg>'


def preview_box(expression, summary, action='daily-backup.sh'):
    s=rect(1068, 265, 273, 572, '#f2f7ec', '#dfe8d3') + text(1087, 291, 'LIVE PREVIEW', 10, '#a6b997', mono=True)
    s += text(1087, 327, '执行计划', 12, '#79975f') + text(1087, 363, expression, 20, '#85a66b', mono=True) + text(1087, 393, summary, 10, '#99ae82') + text(1087, 418, 'Asia/Shanghai · 分钟粒度', 9, '#adbd9c') + line(1087, 440, 1320, 440)
    s += text(1087, 471, '未来执行时间', 12, '#79975f')
    for i, date in enumerate(['2026/11/23 09:00', '2026/11/24 09:00', '2026/11/25 09:00']):
        s+=text(1087, 506+i*37, f'0{i+1}', 9, '#b4c3a4', mono=True)+text(1112, 506+i*37, date, 11, '#96ad7f', mono=True)
    s+=text(1087, 624, '展开全部 10 次  ⌄', 10, '#9eb58a')+line(1087, 647, 1320, 647)+text(1087, 678, '执行动作', 12, '#79975f')+text(1087, 708, action, 11, '#9db48a', mono=True)+text(1087, 744, '启动参数 · 2 项', 10, '#adbd9c')+text(1087, 770, '1. --output', 10, '#9db48a', mono=True)+text(1087, 792, '2. /Backups/项目归档', 10, '#9db48a', mono=True)
    return s


def steps(active):
    s=rect(234, 265, 814, 46)
    for i,label in enumerate(['基本信息','执行计划','执行动作','运行策略']):
        x=234+i*203.5
        if i==active: s+=rect(x,265,203.5,46,'#eaf3e1',LINE,0)
        s+=text(x+28,293,f'0{i+1}   {label}',11,'#739359' if i==active else '#a8b99a')
    return s


def cron_board():
    s=frame('新建任务','把执行时间与具体动作，组合成一个可靠的计划。','02')+steps(1)+preview_box('0 9 * * 1-5','周一至周五 09:00 执行')
    s+=rect(234,329,814,479)+text(259,359,'02 / EXECUTION SCHEDULE',9,'#a7ba92',mono=True)+text(259,393,'什么时候执行？',17,'#6f8e55',500)+text(259,418,'从常用周期开始，表达式与时间预览实时更新。',11,'#a3b690')
    for i,label in enumerate(['常用周期','自定义字段','高级表达式']):s+=rect(259+i*249,444,249,35,'#edf4e4' if i==0 else '#f5f8f0',LINE,3)+text(338+i*249,467,label,11,'#7b9c60' if i==0 else '#adbd9c')
    s+=field(259,514,364,'重复周期','每周                         ⌄')+field(641,514,381,'执行时间','09:00                         ◷')+text(259,597,'每周执行日期',11,'#95ad7d')
    for i,label in enumerate(['周一','周二','周三','周四','周五','周六','周日']):s+=rect(259+i*86,614,77,35,'#eaf3e1' if i<5 else '#fcfdfb', '#aeca95' if i<5 else LINE,4)+text(280+i*86,637,label,11,'#7f9f62' if i<5 else '#b2c0a1')
    s+=text(259,678,'周一至周五不等于法定工作日，不包含节假日与调休。',10,'#b0bf9f')+field(259,715,763,'计划时区','Asia/Shanghai · 北京时间')+button(842,832,98,'上一步')+button(949,832,99,'下一步 →',True)
    return s+'</svg>'


def action_board():
    s=frame('新建任务','配置明确的动作与参数，让每次执行都如你所期。','03')+steps(2)+preview_box('0 18 * * 1-5','周一至周五 18:00 执行')
    s+=rect(234,329,814,520)+text(259,359,'03 / EXECUTION ACTION',9,'#a7ba92',mono=True)+text(259,392,'计划到了，要做什么？',17,'#6f8e55',500)
    for i,(symbol,label,note) in enumerate([('>_','运行脚本','sh / bash / zsh'),('⌘','运行程序','exe / 原生可执行文件'),('▣','打开应用','app / GUI 应用')]):
        x=259+i*256
        s+=rect(x,418,245,78,'#edf5e5' if i==0 else '#fff','#acc596' if i==0 else LINE)+text(x+14,444,symbol,18,'#8ba973',mono=True)+text(x+48,444,label,12,'#849e6a')+text(x+48,467,note,9,'#b0bf9e')
    s+=field(259,530,610,'脚本解释器','/bin/bash')+button(882,542,139,'选择示例')+field(259,601,610,'脚本文件','/Users/developer/Scripts/daily-backup.sh')+button(882,613,139,'选择示例文件')
    s+=text(259,692,'启动参数',13,'#89a570')+text(922,692,'＋ 添加参数',11,'#91ad75')
    for i,value in enumerate(['--output','/Users/developer/Backups/项目归档']):
        y=712+i*44
        s+=text(264,y+24,f'0{i+1}',10,'#b4c3a4',mono=True)+rect(293,y,539,34,'#fcfdfb')+text(305,y+23,value,11,'#99ad85',mono=True)+text(848,y+23,'□ 敏感',9,'#aabf96')+text(939,y+23,'↑   ↓   ×',12,'#a9bb94')
    s+=text(259,826,'每行一个独立参数，支持空参数；不自动展开变量或管道。',10,'#acbd9a')
    return s+'</svg>'


def history_board():
    s=frame('执行记录','每次触发，都留下一个清楚的结果。','04')
    s+=text(235,284,'任务列表  06',12,'#b0bd9f')+text(351,284,'执行记录  06',13,'#7d9b64',500)+line(234,298,1341,298)
    s+=field(234,321,240,'','⌕   搜索任务名称…')+field(489,321,140,'','全部任务   ⌄')+field(643,321,140,'','全部执行结果   ⌄')
    s+=rect(234,386,1107,410)
    for x,label in [(250,'任务名称'),(465,'触发时间'),(620,'来源'),(737,'执行结果'),(909,'耗时 / 退出码'),(1120,'操作')]:s+=text(x,412,label,10,'#a4b696')
    for i,(name,status) in enumerate([('每日项目备份','执行成功'),('打开开发工作区','启动请求成功'),('生成每日构建报告','执行失败'),('更新项目索引','执行中'),('每周清理临时文件','执行成功')]):
        y=457+i*63
        s+=text(250,y,name,12,'#8ca675')+text(250,y+18,'配置 v1 · 计划触发',9,'#b6c4a7')+text(465,y,'11/20 09:00',10,'#a1b68c',mono=True)+text(620,y,'计划触发',10,'#a9bb96')+badge(737,y,status,'#b48d72' if i==2 else '#95af7d',width=100)+text(909,y,'—' if i==1 else '2.4 s / 0',10,'#a8bb94',mono=True)+text(1120,y,'查看详情 ↗',11,'#93ad7a')+line(246,y+31,1328,y+31)
    # Drawer is deliberately part of this static screen design, not a captured running application.
    s+='<rect x="205" y="130" width="1175" height="761" fill="#374829" opacity=".12"/>'
    s+=rect(815,130,565,761,'#fcfdfb',LINE,0)+text(842,164,'RUN DETAIL / SIMULATED',10,'#a4b991',mono=True)+text(842,203,'执行详情',24,'#73905b',500)+text(1334,169,'×',22,'#a8ba96')+text(842,234,'打开开发工作区 · 2026/11/20 09:00:00',11,'#a3b58f')+badge(842,270,'启动请求成功','#a28ab4','#f2edf7',110)
    for i,(label,value) in enumerate([('触发来源','计划示例'),('执行耗时','—'),('退出码','不适用')]):
        x=842+i*168
        s+=rect(x,303,153,67,'#f0f6e8')+text(x+12,326,label,10,'#acbd99')+text(x+12,353,value,13,'#92ac79')
    s+=text(842,410,'计划时间',11,'#afc09c')+text(957,410,'2026/11/20 09:00:00',11,'#9cb487')+text(842,449,'配置版本',11,'#afc09c')+text(957,449,'v1 · 使用触发时快照',11,'#9cb487')
    s+=text(842,501,'标准输出',12,'#8eaa74')+text(948,501,'错误输出',12,'#b3c2a2')+text(1056,501,'配置快照',12,'#b3c2a2')+line(842,516,1350,516)+rect(842,535,508,210,'#f0f6e9')+text(859,565,'[demo] 应用启动请求已被接受。',12,'#9bb284',mono=True)+text(859,598,'本记录不代表应用内部业务已完成。',11,'#a9ba98')+text(842,778,'应用启动请求不等待 GUI 应用退出。',10,'#adbe9b')+button(1113,820,91,'关闭详情')+button(1215,820,135,'再次模拟执行',True)
    return s+'</svg>'


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--png',action='store_true',help='Also render PNG boards with macOS Quick Look')
    args=parser.parse_args()
    OUT.mkdir(parents=True,exist_ok=True)
    for name,build in [('01-task-list',list_board),('02-cron-builder',cron_board),('03-action-parameters',action_board),('04-execution-history',history_board)]:
        path=OUT/f'{name}.svg'
        path.write_text(build(),encoding='utf-8')
        if args.png:
            with tempfile.TemporaryDirectory(prefix='handykit-design-') as temporary:
                # Quick Look crops wide SVGs to a square. Render on a square canvas,
                # then export just the design's 1380:920 area from the top-left.
                square=Path(temporary)/path.name
                square.write_text(path.read_text().replace('height="920" viewBox="0 0 1380 920"','height="1380" viewBox="0 0 1380 1380"',1),encoding='utf-8')
                subprocess.run(['qlmanage','-t','-s','1600','-o',temporary,str(square)],check=True,timeout=30,stdout=subprocess.DEVNULL)
                crop_top_png(Path(temporary)/f'{path.name}.png',OUT/f'{name}.png',1067)
        print(path.name)
