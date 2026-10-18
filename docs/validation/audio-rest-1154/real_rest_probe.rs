use pokered_audio::{audio_manager::AudioManager,music_data::MusicId};
use std::sync::Mutex;
static WRITES:Mutex<Vec<(u16,u8)>>=Mutex::new(Vec::new());
fn record(a:u16,v:u8){WRITES.lock().unwrap().push((a,v));}
fn main(){
 for music in [MusicId::DUNGEON3,MusicId::SILPH_CO]{
  let mut m=AudioManager::new();m.apu.write_register(0xff26,0x80);m.play_music(music);m.apu.set_register_sink(Some(record));let mut seen=[false;3];
  for tick in 0..2000 {
   let pointers=m.sequencer.channels.each_ref().map(|c|c.ptr);WRITES.lock().unwrap().clear();m.update_frame();
   for hw in 0..3 {
    let c=&m.sequencer.channels[hw];
    if !seen[hw]&&c.active&&c.ptr!=pointers[hw]&&c.ptr>0&&matches!(c.data.get(c.ptr-1),Some(0xc0..=0xcf)) {
     seen[hw]=true;let range=[0xff10..=0xff14,0xff16..=0xff19,0xff1a..=0xff1e][hw].clone();let writes:Vec<_>=WRITES.lock().unwrap().iter().copied().filter(|(a,_)|range.contains(a)).collect();
     println!("{}",serde_json::json!({"music":format!("{music:?}"),"tick":tick,"channel":hw,"ptr":c.ptr,"rest_byte":c.data[c.ptr-1],"logical_volume":c.volume_envelope,"note_delay":c.delay_counter,"vibrato_delay":c.vibrato.delay_counter,"vibrato_reload":c.vibrato.delay_reload,"NR51":m.apu.nr51,"writes":writes}));
    }
   }
   if seen.iter().all(|x|*x){break;}
  }
  eprintln!("{music:?} rest coverage {seen:?}");
 }
}
