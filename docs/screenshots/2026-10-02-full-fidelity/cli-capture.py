from pathlib import Path
import subprocess,sys
binary,prefix=sys.argv[1:3];out=Path(__file__).resolve().parent
cases=[('copyright',0),('gamefreak-splash',181),('gamefreak-splash',244),('gamefreak-splash',284),('title',121),('title',125),('title',441),('title',507),('title',563),('title',900),('trainer-card',5),('diploma',5),('hof',0),('hof',220),('hof',480),('hof',488),('credits',5),('credits',10),('credits',15),('credits',388),('credits',6600)]
for screen,frame in cases:
 p=out/f'{prefix}-{screen}-{frame}.png'
 r=subprocess.run([binary,'screenshot','--screen',screen,'--frames',str(frame),'--lang','en','-o',str(p)],capture_output=True,text=True)
 if r.returncode:print(screen,frame,r.stderr[-1000:]);raise SystemExit(r.returncode)
print(prefix,len(cases),'screenshots captured')
