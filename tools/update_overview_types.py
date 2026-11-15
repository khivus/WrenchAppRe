"""Rebuild actual overview object names from CCP official JSONL SDE (run at repository root)."""
import urllib.request,zipfile,json,tempfile,pathlib
url='https://developers.eveonline.com/static-data/eve-online-static-data-latest-jsonl.zip'
p=pathlib.Path(tempfile.gettempdir())/'wrench-preview-sde.zip'
if not p.exists(): urllib.request.urlretrieve(url,p)
groups={int(i) for c in json.load(open('assets/overview-categories.json'))['categories'].values() for i in c['groups']}
result={}
with zipfile.ZipFile(p) as z:
 for line in z.open('types.jsonl'):
  t=json.loads(line); g=t.get('groupID'); name=t.get('name',{}).get('en')
  if g in groups and name and not any(x in name.lower() for x in ('[no messageid:', 'test', 'qa ')): result.setdefault(str(g),[]).append(name)
pathlib.Path('assets/overview-types.json').write_text(json.dumps(result,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
print('Catalog:',len(result),'groups;',sum(map(len,result.values())),'types')
