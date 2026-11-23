"""Shared semantic palette adapter for static design-board renderers."""
from pathlib import Path
import json

SOURCE=Path(__file__).resolve().parents[1]/'development/design-system/tokens.json'
TOKENS=json.loads(SOURCE.read_text(encoding='utf-8'))


def resolve(palette,color,role='text'):
    if color in palette:return palette[color]
    if color.lower()=='#315f49':return TOKENS['shared']['brand-green']
    if not color.startswith('#'):return color
    value=color[1:]
    if len(value)==3:value=''.join(character*2 for character in value)
    rgb=[int(value[index:index+2],16)/255 for index in (0,2,4)]
    level=sum(rgb)/3
    if role=='text':
        if min(rgb)>.96:return palette['on-accent']
        return palette['text-primary' if level<.5 else 'text-secondary' if level<.68 else 'text-tertiary']
    if role=='border':return palette['separator']
    exact={
        '#fff':'surface','#ffffff':'surface','#f9fbf7':'canvas','#f1f5ec':'sidebar',
        '#edf2e8':'toolbar','#f1f6eb':'toolbar','#f7f3e9':'warning-bg',
        '#e3eddb':'selection','#eaf3e1':'selection','#edf5e5':'selection',
        '#edf4e4':'selection','#4d7951':'accent','#fcfdfb':'field',
        '#f2f7ec':'surface','#f0f6e9':'surface-inset','#f0f6e8':'surface-inset',
        '#f0f5e9':'toolbar','#f5f8f0':'surface-inset','#eef5e6':'success-bg',
    }
    return palette[exact.get(color.lower(),'surface-inset')]
