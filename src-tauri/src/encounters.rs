//! Short, interruptible events. Waiting time is eligible visible idle time, never a catch-up queue.
use crate::{atlas::Row, engine::Input};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EncounterView { pub kind: Option<&'static str>, pub phase: &'static str, pub right: bool }

pub struct Encounters {
    active: Option<bool>, // true: ball invitation; false: butterfly
    elapsed: u64,
    eligible: u64,
    delay: u64,
    origin: (i32,i32),
    right: bool,
    next_ball: bool,
    rng: u64,
}

pub struct Step {
    pub movement: Option<(i32,i32)>,
    pub row: Option<Row>,
    pub look: Option<u8>,
    pub offer_ball: bool,
    pub cancel_ball: bool,
}

impl Encounters {
    pub fn new(seed:u64) -> Self {
        Self { active:None,elapsed:0,eligible:0,delay:interval(seed),origin:(0,0),right:true,
            next_ball:seed%2==0,rng:seed|1 }
    }
    pub fn active(&self)->bool { self.active.is_some() }
    pub fn offering_ball(&self)->bool { self.active==Some(true) }
    pub fn interrupt(&mut self) { self.active=None;self.elapsed=0;self.eligible=0; }
    fn reset(&mut self) {
        self.interrupt();self.rng^=self.rng<<13;self.rng^=self.rng>>7;self.rng^=self.rng<<17;
        self.delay=interval(self.rng);
    }
    pub fn view(&self)->EncounterView {
        match self.active {
            None=>EncounterView{kind:None,phase:"quiet",right:self.right},
            Some(true)=>EncounterView{kind:Some("ball"),phase:if self.elapsed<1800 { "pushing" } else { "inviting" },right:self.right},
            Some(false)=>EncounterView{kind:Some("butterfly"),phase:if self.elapsed<2000 { "watching" } else if self.elapsed<4000 { "following" } else { "departing" },right:self.right},
        }
    }
    pub fn tick(&mut self,input:&Input,hard_blocked:bool,eligible:bool)->Step {
        let mut out=Step{movement:None,row:None,look:None,offer_ball:false,cancel_ball:false};
        if !input.encounters_enabled || hard_blocked {
            out.cancel_ball=self.offering_ball();self.reset();return out;
        }
        if self.active.is_none() {
            self.eligible=self.eligible.saturating_add(input.dt_ms.min(120));
            // A long suspend contributes at most one normal frame, never an instant event.
            if self.eligible<self.delay || !eligible { return out; }
            self.origin=input.win_pos;
            self.right=input.win_pos.0+input.win_size.0/2<input.screen.0+input.screen.2/2;
            self.active=Some(self.next_ball);self.next_ball=!self.next_ball;self.elapsed=0;
            out.offer_ball=self.offering_ball();
        } else {
            self.elapsed=self.elapsed.saturating_add(input.dt_ms);
        }
        let ball=self.offering_ball();
        if self.elapsed>=if ball { 7000 } else { 6000 } {
            out.cancel_ball=ball;self.reset();return out;
        }
        let distance=if ball { 24.0 } else { 48.0 };
        let progress=if ball { (self.elapsed as f32/1800.0).min(1.0) }
            else { (self.elapsed.saturating_sub(2000) as f32/2000.0).min(1.0) };
        if progress>0.0 && progress<1.0 {
            let target=self.origin.0+(distance*input.scale_factor as f32*progress*if self.right { 1.0 } else { -1.0 }).round() as i32;
            let (sx,sy,sw,sh)=input.screen;
            if sw>=input.win_size.0 && sh>=input.win_size.1 {
                out.movement=Some((target.clamp(sx,sx+sw-input.win_size.0),self.origin.1.clamp(sy,sy+sh-input.win_size.1)));
            }
            out.row=Some(if self.right { Row::RunRight } else { Row::RunLeft });
        } else if ball {
            out.row=Some(Row::Waving);
        } else {
            out.look=Some(if self.right { 2 } else { 14 });
            out.row=Some(if input.look_enabled { if self.right { Row::LookA } else { Row::LookB } } else { Row::Waiting });
        }
        out
    }
}

fn interval(seed:u64)->u64 {
    // Only explicitly enabled debug test builds shorten idle waits. Release builds always use production timing.
    if cfg!(all(feature="e2e",debug_assertions)) { 3000+seed%1000 } else { 90_000+seed%60_000 }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input()->Input { Input{dt_ms:100,interactive:true,cursor:(-4000,-4000),win_pos:(100,100),win_size:(300,240),
        scale_factor:1.0,screen:(0,0,1280,800),button_down:false,look_enabled:true,extra_animations:true,
        gravity:false,local_hour:12,sleep_frame:(Row::Failed,2),encounters_enabled:true} }
    #[test]
    fn blocked_idle_never_accumulates_and_suspend_does_not_catch_up() {
        let mut events=Encounters::new(1);let mut i=input();i.dt_ms=600_000;
        for _ in 0..20 { events.tick(&i,true,true);assert!(!events.active()); }
        events.tick(&i,false,true);assert!(!events.active());assert_eq!(events.eligible,120);
        i.encounters_enabled=false;events.tick(&i,false,true);assert_eq!(events.eligible,0);
    }
    #[test]
    fn both_events_finish_and_user_activity_cancels_immediately() {
        let mut events=Encounters::new(1);let i=input();let mut kinds=Vec::new();
        for _ in 0..2 {
            events.eligible=events.delay;let first=events.tick(&i,false,true);kinds.push(events.view().kind.unwrap());
            assert_eq!(first.offer_ball,events.offering_ball());
            for _ in 0..80 { events.tick(&i,false,false); }
            assert!(!events.active());
        }
        assert!(kinds.contains(&"ball") && kinds.contains(&"butterfly"));
        events.eligible=events.delay;events.tick(&i,false,true);events.tick(&i,true,true);assert!(!events.active());
    }
    #[test]
    fn event_motion_respects_negative_work_area_at_scaled_dpi() {
        let mut events=Encounters::new(1);let mut i=input();i.scale_factor=2.0;i.win_size=(600,480);i.screen=(-1280,20,1280,760);i.win_pos=(-610,200);
        events.eligible=events.delay;events.tick(&i,false,true);
        for _ in 0..60 { let out=events.tick(&i,false,false);if let Some((x,y))=out.movement { assert!((-1280..=-600).contains(&x));assert!((20..=300).contains(&y));i.win_pos=(x,y); } }
    }
}
