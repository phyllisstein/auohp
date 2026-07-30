import re, json, sys

SRC="/home/ubuntu/108_truth.txt"
OUT="/home/ubuntu/auohp/packages/core/tests/fixtures/108_truth.clean.txt"

raw=open(SRC,encoding='utf-8').read().replace('\ufeff','')
lines=[l.rstrip() for l in raw.split('\n')]

stats={}
def bump(k,n=1): stats[k]=stats.get(k,0)+n

# --- 1. excise furniture that is embedded INSIDE speech lines (two-column layout) ---
INLINE=[
    (r'^Tape III\t', ''),                       # 105: marker in left margin column
    (r'^Tape III original sketches', 'original sketches'),   # 235
    (r'\s*00:30:00\s*$', ''),                   # 235 trailing
    (r'^p incredibly', 'incredibly'),           # 265: 'p' is OCR debris from 'Tape'
    (r'\s*00:35:00\s*J\s*$', ''),               # 265 trailing
]
for i,l in enumerate(lines):
    for pat,rep in INLINE:
        new=re.sub(pat,rep,l)
        if new!=l: bump('inline furniture excised'); l=new
    lines[i]=l

# --- 2. drop whole-line furniture ---
DROP=[
    (r'^Avram Finkelstein Interview(\s*\d+)?$','page header'),
    (r'^January 23, 2010$','page header'),
    (r'^\d{1,3}$','page number'),
    (r'^Ta[pn]e (III|IV)(\s+\d\d:\d\d:\d\d)?$','tape marker'),
    (r'^\d\d:\d\d:\d\d(\s*[A-Z])?$','tape marker'),
    (r'^[A-Z]$','OCR debris'),
]
kept=[]
for l in lines:
    s=l.strip()
    if not s: continue
    hit=next((why for pat,why in DROP if re.match(pat,s)),None)
    if hit: bump(hit); continue
    kept.append(s)

text=' '.join(kept)

# --- 3. speaker tags, stage directions, editorial insertions ---
text,n=re.subn(r'(?:^|(?<=\s))(?:AF|SS|88|JW):\s*','',text); bump('speaker tags',n)
text,n=re.subn(r'\{[^}]*\}','',text);                        bump('stage directions',n)
text,n=re.subn(r'\[[^\]]*\]\s*','',text);                    bump('editorial insertions',n)

# --- 4. unambiguous OCR repairs ---
REPAIR=[(r'hi erar chai','hierarchical'), (r'\b88:1 remember\b','I remember'), (r'\b1 remember\b','I remember')]
for pat,rep in REPAIR:
    text,n=re.subn(pat,rep,text)
    if n: bump(f'OCR repair {pat!r} -> {rep!r}',n)

text=re.sub(r'\s+',' ',text).strip()

# wrap for reviewability; word stream is unaffected
import textwrap
open(OUT,'w',encoding='utf-8').write('\n'.join(textwrap.wrap(text,width=96,break_on_hyphens=False,break_long_words=False))+'\n')

print(f"words: {len(text.split())}   chars: {len(text)}")
for k,v in sorted(stats.items()): print(f"  {v:4}  {k}")
