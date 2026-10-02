from pathlib import Path
import json, os, re, subprocess, sys
artifacts, deps, source, output = sys.argv[1:5]
if artifacts.endswith(".rlib"):
    # Match the exact feature/engine versions in the app, instead of picking
    # the newest core/data library from an unrelated target build.
    result=subprocess.run(["rustc","-Z","ls=all",artifacts],check=True,capture_output=True,text=True,
        env={**os.environ,"RUSTC_BOOTSTRAP":"1"})
    records=[]
    for name in ["pokered_app","pokered_core","pokered_data","pokered_renderer","dotzuki_engine"]:
        tag=re.search(r"\b"+name+r"-[0-9a-f]+\b",result.stdout).group()
        records.append({"reason":"compiler-artifact","target":{"name":name},"filenames":[str(Path(deps)/("lib"+tag+".rlib"))]})
else:
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
