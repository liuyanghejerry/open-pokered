use pokered_audio::{audio_manager::AudioManager,music_data::MusicId,sfx_data::SfxId};
use std::sync::Mutex;
static WRITES: Mutex<Vec<(u16,u8)>>=Mutex::new(Vec::new());
fn record(a:u16,v:u8){WRITES.lock().unwrap().push((a,v));}
fn main(){
 for music in [MusicId::DUNGEON3,MusicId::SILPH_CO]{
  let mut m=AudioManager::new();m.apu.write_register(0xff26,0x80);m.play_music(music);m.update_frame();m.play_sfx(SfxId::PressAB);m.apu.set_register_sink(Some(record));
  for tick in 0..20{
   WRITES.lock().unwrap().clear();let pre=m.sequencer.channels[0].clone();let active=m.sequencer.channels[4].active;m.update_frame();
   println!("{}",serde_json::json!({"music":format!("{music:?}"),"tick":tick,"pre_sfx":active,"post_sfx":m.sequencer.channels[4].active,"pre_music_delay":pre.delay_counter,"pre_vibrato_delay":pre.vibrato.delay_counter,"pre_vibrato_rate":pre.vibrato.rate,"pre_music_trigger":pre.trigger,"post_music_delay":m.sequencer.channels[0].delay_counter,"writes":*WRITES.lock().unwrap()}));
  }
 }
}
