//! VBlank hardware acceptance and the fixed $0040 JP VBlank vector.
//! A pending interrupt survives LCD disable. Entry is NOT the ISR body/RETI;
//! the foreground remains suspended when this child reaches BodyReady.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
pub trait VBlankEntryBus: DisplayInstructionBus {
    fn enter_vblank(&mut self) -> Result<(), Self::Error>;
}
impl VBlankEntryBus for super::display_work_time::DisplayWorkTime {
    fn enter_vblank(&mut self) -> Result<(), Self::Error> {
        self.enter_pending_vblank()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Accept,
    Vector,
    BodyReady,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankEntryWork {
    step: Step,
}
impl VBlankEntryWork {
    pub fn select(ime: bool, pending: u8) -> Option<Self> {
        (ime && pending & 1 != 0).then_some(Self { step: Step::Accept })
    }
    pub fn body_ready(&self) -> bool {
        self.step == Step::BodyReady
    }
    pub fn step<B: VBlankEntryBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        self.step = match self.step {
            Step::Accept => {
                bus.enter_vblank()?;
                Step::Vector
            }
            Step::Vector => {
                bus.execute(DisplayInstruction {
                    tcycles: 16,
                    access: None,
                })?;
                Step::BodyReady
            }
            Step::BodyReady => return Ok(false),
        };
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_lcd_work::DisplayAccess;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    fn write(t: &mut DisplayWorkTime, port: u16, value: u8) {
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle: 8,
            }),
        })
        .unwrap();
    }
    #[test]
    fn masked_edge_is_accepted_after_ie_restore_even_with_lcd_off() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        write(&mut t, 0xffff, 0);
        t.idle_to_target().unwrap();
        assert_eq!(t.device().read_port(0xff0f).unwrap() & 1, 1);
        assert!(VBlankEntryWork::select(true, t.device().pending_interrupts()).is_none());
        t.begin_native_update();
        write(&mut t, 0xff40, 0x11);
        write(&mut t, 0xffff, 1);
        assert!(!t.device().lcd_enabled());
        let start = t.elapsed();
        let mut w = VBlankEntryWork::select(true, t.device().pending_interrupts()).unwrap();
        assert!(w.step(&mut t).unwrap());
        assert_eq!(t.elapsed() - start, 20);
        assert_eq!(t.device().read_port(0xff0f).unwrap() & 1, 0);
        assert!(t.active_vblank());
        assert!(w.step(&mut t).unwrap());
        assert!(w.body_ready());
        assert_eq!(t.elapsed() - start, 36);
        assert!(!w.step(&mut t).unwrap());
        assert_eq!(
            super::super::display_delay_work::DelayFrameBus::interrupt_returns(&t),
            0
        );
        assert_eq!(
            t.enter_pending_vblank(),
            Err(DisplayWorkError::InterruptBusy)
        );
    }
    #[test]
    fn exhausted_budget_cannot_acknowledge_or_lose_the_request() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.idle_to_target().unwrap();
        let mut w = VBlankEntryWork::select(true, t.device().pending_interrupts()).unwrap();
        let before = w;
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, before);
        assert_eq!(t.device().pending_interrupts() & 1, 1);
        assert!(!t.active_vblank());
        assert!(VBlankEntryWork::select(false, t.device().pending_interrupts()).is_none());
    }
    #[test]
    fn snapshot_keeps_active_entry_and_does_not_fabricate_a_return() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.idle_to_target().unwrap();
        t.begin_native_update();
        let mut w = VBlankEntryWork::select(true, t.device().pending_interrupts()).unwrap();
        w.step(&mut t).unwrap();
        let (mut other, mut owner): (VBlankEntryWork, DisplayWorkTime) =
            serde_json::from_str(&serde_json::to_string(&(&w, &t)).unwrap()).unwrap();
        while w.step(&mut t).unwrap() {}
        while other.step(&mut owner).unwrap() {}
        assert_eq!(w, other);
        assert_eq!(t.elapsed(), owner.elapsed());
        assert!(owner.active_vblank());
        assert_eq!(
            super::super::display_delay_work::DelayFrameBus::interrupt_returns(&owner),
            0
        );
    }
}
