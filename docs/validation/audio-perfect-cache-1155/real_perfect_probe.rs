use pokered_audio::{audio_manager::AudioManager,music_data::MusicId,ChannelFlags1};
use std::sync::Mutex;
static WRITES:Mutex<Vec<(u16,u8)>>=Mutex::new(Vec::new());
fn record(a:u16,v:u8){WRITES.lock().unwrap().push((a,v));}
fn main(){for music in [MusicId::DUNGEON3,MusicId::SILPH_CO]{
 let mut m=AudioManager::new();m.apu.write_register(0xff26,0x80);m.play_music(music);m.apu.set_register_sink(Some(record));let mut onsets=0;let mut effect=false;
 for tick in 0..2000 {WRITES.lock().unwrap().clear();m.update_frame();let c=&m.sequencer.channels[0];if !c.active||!c.flags1.contains(ChannelFlags1::PERFECT_PITCH){continue;}
 let w=WRITES.lock().unwrap().clone();let low=w.iter().rev().find_map(|&(a,v)|(a==0xff13).then_some(v));let onset=w.iter().any(|&(a,v)|a==0xff14&&v&128!=0);
 let mode=if onset&&onsets<3 {onsets+=1;Some("onset")}else if !effect&&c.vibrato_freq_lo.is_some()&&low.is_some(){effect=true;Some("vibrato")}else{None};
 if let Some(mode)=mode{println!("{}",serde_json::json!({"music":format!("{music:?}"),"tick":tick,"mode":mode,"frequency":c.frequency,"saved_low":c.freq_lo_saved,"written_low":low,"delay":c.delay_counter,"ptr":c.ptr,"slide_origin":c.pitch_slide.current_freq}));}
 if onsets==3&&effect{break;}
 }
 assert_eq!(onsets,3);assert!(effect);
}}
