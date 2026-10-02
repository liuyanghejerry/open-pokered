from pathlib import Path
import json, subprocess, sys
artifacts, deps, source, output = sys.argv[1:5]
raw=Path(artifacts).read_text()
try: records=json.loads(raw)
except json.JSONDecodeError: records=[json.loads(line) for line in raw.splitlines() if line]
records=[a for a in records if a.get("reason")=="compiler-artifact"]
args=["rustc","--edition=2021",source,"-L","dependency="+deps,"-L","native=/workspace/onboarding/sysroot/usr/lib/x86_64-linux-gnu"]
for name in ["pokered_app","pokered_core","pokered_data","pokered_renderer","dotzuki_engine"]:
    paths=[p for a in records if a["target"]["name"]==name for p in a.get("filenames",[]) if p.endswith(".rlib")]
    assert paths,name
    lib=str(Path(deps)/Path(paths[-1]).name)
    args += ["--extern",name+"="+lib]
if len(sys.argv)>5 and sys.argv[5]=="after": args += ["--cfg","fidelity_after"]
args += ["-o",output]
subprocess.run(args,check=True)
