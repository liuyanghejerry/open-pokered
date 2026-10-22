#!/usr/bin/env python3
"""Controlled channel-end source-tail fixtures. Not a gameplay recording."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('rom',type=Path);p.add_argument('symbols',type=Path);p.add_argument('output',type=Path);args=p.parse_args()
    spec=importlib.util.spec_from_file_location('oracle',Path(__file__).with_name('generate-timer-instructions.py'));o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
    rom=args.rom.read_bytes();assert hashlib.sha256(rom).hexdigest()==o.ROM_SHA
    symbols={}
    for line in args.symbols.read_text().splitlines():
        bits=line.split()
        if len(bits)>1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank,address=bits[0].split(':');symbols[bits[1]]=(int(bank,16),int(address,16))
    output=[]
    for prefix in ('Audio1','Audio3'):
        bank,ret=symbols[prefix+'_sound_ret'];entry=ret+19
        start=bank*0x4000+entry-0x4000;assert rom[start:start+4]==bytes([0x79,0xfe,3,0x30])
        for ch in range(8):
            for sid in (0,19,20,133,134,144,255):
                if ch==4 and sid==0:continue
                for disabled in (0,1):
                    for ptr in (0,0x100,0xd101):
                        own=sid if ch==4 else 0xba
                        i=dict(bank=bank,channel=ch,source_sfx_id=sid,flags1=0x4c,flags2=0x81,disable_output=disabled,saved_volume=0x53,panning=255,command_ptr=ptr)
                        memory={symbols['wChannelSoundIDs'][1]+ch:own,symbols['wChannelSoundIDs'][1]+4:sid,symbols['wChannelFlags1'][1]+ch:i['flags1'],symbols['wChannelFlags2'][1]+ch:i['flags2'],symbols['wDisableChannelOutputWhenSfxEnds'][1]:disabled,symbols['wSavedVolume'][1]:i['saved_volume'],0xff25:i['panning']}
                        a=symbols['wChannelCommandPointers'][1]+ch*2;memory[a]=ptr&255;memory[a+1]=ptr>>8
                        trace,_,m=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch,'H':(symbols['wChannelFlags1'][1]+ch)>>8,'L':(symbols['wChannelFlags1'][1]+ch)&255},memory=memory,capture=True)
                        def wrote(address):
                            return any(rom[bank*0x4000+pc-0x4000]==0xea and int.from_bytes(rom[bank*0x4000+pc-0x4000+1:bank*0x4000+pc-0x4000+3],'little')==address for pc,_ in trace)
                        pan_write=symbols[prefix+'_sound_ret.disableChannelOutput'][1]+7
                        assert rom[bank*0x4000+pan_write-0x4000:bank*0x4000+pan_write-0x4000+2]==bytes([0xe0,0x25])
                        rewound=20<=sid<134 and ch!=4
                        result=dict(ended=m[symbols['wChannelSoundIDs'][1]+ch]==0,flags1=m[symbols['wChannelFlags1'][1]+ch],flags2=m[symbols['wChannelFlags2'][1]+ch],disable_output_write=0 if wrote(symbols['wDisableChannelOutputWhenSfxEnds'][1]) else None,clear_saved_volume=wrote(symbols['wSavedVolume'][1]),panning_write=m[0xff25] if any(pc==pan_write for pc,_ in trace) else None,command_ptr_write=(m[a]|m[a+1]<<8) if rewound else None,wave_restarted=0xff1a in m,volume_restore=m.get(0xff24))
                        output.append(dict(input=i,result=result,trace=trace))
    text='{\n  "scope":"Controlled sound_ret tail CPU fixtures with explicit source pointer word; no gameplay, APU timing or canonical pointer producer claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'+',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in output)+'\n  ]\n}\n';args.output.write_text(text);print(len(output),'channel-ending tails')

if __name__=='__main__':main()
