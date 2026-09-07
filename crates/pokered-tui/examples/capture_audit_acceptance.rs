#[path="../src/render/mod.rs"] mod render;
include!("../../../docs/audits/2026-09-07/capture-acceptance.rs");

fn capture_overworld(s: &mut OverworldScreen, r: &mut Option<ResourceManager>, f: &mut FrameBuffer) { render::draw_overworld(s,r,f); }
