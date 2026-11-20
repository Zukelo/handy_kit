"""Check semantic color pairs, token copies and legacy hardcoded color regressions."""
from pathlib import Path
import json
import re
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
TOKENS=json.loads((ROOT/'development/design-system/tokens.json').read_text(encoding='utf-8'))


def luminance(color):
    channels=[int(color[index:index+2],16)/255 for index in (1,3,5)]
    linear=[value/12.92 if value<=0.04045 else ((value+0.055)/1.055)**2.4 for value in channels]
    return sum(a*b for a,b in zip(linear,[0.2126,0.7152,0.0722]))


def contrast(a,b):
    light,dark=sorted([luminance(a),luminance(b)],reverse=True)
    return (light+0.05)/(dark+0.05)


def measurements():
    result=[]
    for mode,palette in TOKENS['themes'].items():
        for foreground in ['text-primary','text-secondary','text-tertiary']:
            for background in ['canvas','sidebar','toolbar','surface','surface-raised','surface-inset','field','hover','selection']:
                result.append((mode,foreground,background,4.5,contrast(palette[foreground],palette[background])))
        for foreground,background in [('accent-text','surface'),('accent-text','canvas'),('selection-text','selection'),('on-accent','accent'),('on-accent','accent-hover'),('on-accent','accent-active'),('success','success-bg'),('warning','warning-bg'),('danger','danger-bg'),('info','info-bg'),('purple','purple-bg'),('code-text','surface-inset')]:
            result.append((mode,foreground,background,4.5,contrast(palette[foreground],palette[background])))
        for foreground in ['control-border','focus']:
            for background in ['field','surface','canvas','sidebar']:
                result.append((mode,foreground,background,3,contrast(palette[foreground],palette[background])))
    return result


if __name__=='__main__':
    subprocess.run([sys.executable,str(ROOT/'scripts/sync_design_system.py'),'--check'],check=True)
    rows=measurements()
    failures=[row for row in rows if row[4]<row[3]]
    for path in ['development/apps/desktop/src/style.css','development/apps/desktop/src/styles/workbench.css','docs/prototypes/tasks/tasks.css','development/design-system/components.css','development/apps/desktop/src/tasks/tasks.css']:
        content=(ROOT/path).read_text()
        if re.search(r'#[0-9a-fA-F]{3,8}\b|rgba?\(',content):failures.append(('hardcoded color',path))
    if failures:
        for row in failures:print('FAIL',*row)
        raise SystemExit(1)
    for mode in TOKENS['themes']:
        text=[row[4] for row in rows if row[0]==mode and row[3]==4.5]
        controls=[row[4] for row in rows if row[0]==mode and row[3]==3]
        print(f'{mode}: minimum specified text-pair contrast {min(text):.2f}:1; control/focus {min(controls):.2f}:1')
    print(f'PASS: {len(rows)} specified theme color pairs; generated copies and stylesheet token usage')
