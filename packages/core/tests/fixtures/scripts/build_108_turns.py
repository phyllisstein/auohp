import re,json
raw=open("/home/ubuntu/108_truth.txt",encoding='utf-8').read().replace('\ufeff','')
lines=[l.rstrip() for l in raw.split('\n')]
INLINE=[(r'^Tape III\t',''),(r'^Tape III original sketches','original sketches'),
        (r'\s*00:30:00\s*$',''),(r'^p incredibly','incredibly'),(r'\s*00:35:00\s*J\s*$','')]
DROP=[r'^Avram Finkelstein Interview(\s*\d+)?$',r'^January 23, 2010$',r'^\d{1,3}$',
      r'^Ta[pn]e (III|IV)(\s+\d\d:\d\d:\d\d)?$',r'^\d\d:\d\d:\d\d(\s*[A-Z])?$',r'^[A-Z]$']
kept=[]
for l in lines:
    for p,r in INLINE: l=re.sub(p,r,l)
    s=l.strip()
    if not s or any(re.match(p,s) for p in DROP): continue
    kept.append(s)
turns=[];cur=None;buf=[]
for s in kept:
    m=re.match(r'^(AF|SS|88|JW):\s*(.*)$',s)
    if m:
        if cur is not None: turns.append({"speaker":cur,"text":' '.join(buf).strip()})
        cur='INTERVIEWER' if m.group(1) in ('SS','88','JW') else 'SUBJECT'
        buf=[m.group(2)]
    else: buf.append(s)
if cur is not None: turns.append({"speaker":cur,"text":' '.join(buf).strip()})

def scrub(t):
    t=re.sub(r'\{[^}]*\}','',t); t=re.sub(r'\[[^\]]*\]\s*','',t)
    t=t.replace('hi erar chai','hierarchical').replace('1 remember','I remember')
    return re.sub(r'\s+',' ',t).strip()
for t in turns: t['text']=scrub(t['text'])
turns=[t for t in turns if t['text']]

joined=' '.join(t['text'] for t in turns)
clean=' '.join(open("/home/ubuntu/auohp/packages/core/tests/fixtures/108_truth.clean.txt",encoding='utf-8').read().split())
assert joined.split()==clean.split(), (
    f"turns must reconstruct the clean truth exactly; {len(joined.split())} vs {len(clean.split())}")

out={"_comment":[
 "Speaker turns for 108, reconstructed from the AF:/SS:/88: tags in the source PDF.",
 "Concatenating `text` across turns reproduces 108_truth.clean.txt token-for-token;",
 "the generator asserts this, so the two fixtures cannot drift apart.",
 "'88:' is an OCR corruption of 'SS:' (Sarah Schulman); both map to INTERVIEWER.",
 "Used to measure whether segment boundaries respect speaker turns -- see",
 "StructureStats::speaker in the scoring harness."],
 "turns":turns}
p="/home/ubuntu/auohp/packages/core/tests/fixtures/108_truth.turns.json"
json.dump(out,open(p,'w'),indent=1)
print(f"{len(turns)} turns, {len(joined.split())} words -> {p}")
from collections import Counter
print(Counter(t['speaker'] for t in turns))
