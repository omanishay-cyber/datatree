import os,sys
BASE=os.path.dirname(os.path.abspath(__file__))
for a in sys.argv[1:]:
  s=os.path.join(BASE,a)
  d=a.replace(".txt",".md")
  c=open(s,encoding="utf-8").read()
  open(os.path.join(BASE,d),"w",encoding="utf-8").write(c)
  print(f"  {d}: {c.count(chr(10))} lines")
  os.remove(s)
print("Done")
