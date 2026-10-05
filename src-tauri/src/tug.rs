//! A mouse-held rope game. Positions are physical pixels; effort uses real time.
use crate::engine::{Input, PET_X, PET_Y};
use serde::Serialize;
include!("../../assets/animation-source/tug-anchors.rs");

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TugView {
    pub mouth: (i32, i32),
    pub handle: (i32, i32),
    pub tension: u8,
    pub right: bool,
}

pub struct Tug {
    pub phase: &'static str,
    home: (i32, i32),
    mouth: (i32, i32),
    handle: (i32, i32),
    right: bool,
    tension: u8,
    was_down: bool,
    elapsed: u64,
    effort: u64,
}

pub struct TugStep {
    pub movement: Option<(i32, i32)>,
    pub completed: bool,
}

impl Tug {
    pub fn new(input: &Input) -> Option<Self> {
        let (sx, sy, sw, sh) = input.screen;
        if !input.interactive || sw < input.win_size.0 || sh < input.win_size.1 {
            return None;
        }
        let home = (input.win_pos.0.clamp(sx, sx + sw - input.win_size.0),
            input.win_pos.1.clamp(sy, sy + sh - input.win_size.1));
        let right = home.0 + input.win_size.0 / 2 < sx + sw / 2;
        let mut tug = Self { phase: "tug_ready", home, mouth: home, handle: home,
            right, tension: 0, was_down: input.button_down, elapsed: 0, effort: 0 };
        tug.align(input, home);
        Some(tug)
    }

    pub fn view(&self) -> Option<TugView> {
        if matches!(self.phase, "off" | "tug_done") { return None; }
        Some(TugView { mouth: self.mouth, handle: self.handle,
            tension: self.tension, right: self.right })
    }

    pub fn pointer_hot(&self, cursor: (i32, i32), scale: f64) -> bool {
        // Holding the rope must never turn into dragging the pet while crossing it.
        self.phase == "tugging" || (self.phase == "tug_ready" &&
            ((cursor.0 - self.handle.0) as f64).hypot((cursor.1 - self.handle.1) as f64) <= 22.0 * scale)
    }

    pub fn direction(&self) -> Option<bool> {
        if matches!(self.phase, "tug_ready" | "tugging") { Some(self.right) } else { None }
    }

    pub fn frame(&self) -> usize {
        if self.phase=="tugging" {((self.elapsed/275)%4) as usize} else {0}
    }

    pub fn align(&mut self, input: &Input, position: (i32, i32)) {
        let s = input.scale_factor as f32;
        // Builtin has a short rope end baked between the jaws; join at its cut tip.
        // Legacy pets retain their existing run-row anchors and never access new rows.
        let anchor=if input.extra_animations {
            let (x,y)=if self.right {TUG_RIGHT[self.frame()]} else {TUG_LEFT[self.frame()]};
            (x*0.75,y*0.75)
        } else {(if self.right {136.5} else {7.5},70.5)};
        self.mouth = (position.0 + ((PET_X as f32 + anchor.0) * s).round() as i32,
            position.1 + ((PET_Y as f32 + anchor.1) * s).round() as i32);
        if self.phase == "tug_ready" {
            let dir = if self.right {1.0} else {-1.0};
            self.handle = (self.mouth.0 + (dir * 80.0 * s).round() as i32, self.mouth.1);
        }
        let (sx, sy, sw, sh) = input.screen;
        let margin = (20.0 * s).round() as i32;
        self.handle.0 = self.handle.0.clamp(sx + margin, sx + sw - margin);
        self.handle.1 = self.handle.1.clamp(sy + margin, sy + sh - margin);
    }

    pub fn tick(&mut self, input: &Input) -> TugStep {
        let mut out = TugStep { movement: None, completed: false };
        let (sx, sy, sw, sh) = input.screen;
        if !input.interactive || input.dt_ms > 1000 || sw < input.win_size.0 || sh < input.win_size.1 {
            self.phase = "off";
            return out;
        }
        let s = input.scale_factor as f32;
        let dir = if self.right {1.0} else {-1.0};
        self.home.0 = self.home.0.clamp(sx, sx + sw - input.win_size.0);
        self.home.1 = self.home.1.clamp(sy, sy + sh - input.win_size.1);
        if self.phase == "tug_done" {
            self.elapsed = self.elapsed.saturating_add(input.dt_ms);
            if self.elapsed >= 1600 { self.phase = "tug_ready"; self.tension = 0; }
        } else if self.phase == "tug_ready" && input.button_down && !self.was_down
            && self.pointer_hot(input.cursor, input.scale_factor) {
            self.phase = "tugging";
            self.home = input.win_pos;
            self.elapsed = 0;
            self.effort = 0;
        }
        if self.phase == "tugging" {
            self.elapsed = self.elapsed.saturating_add(input.dt_ms);
            let distance = (((input.cursor.0 - self.mouth.0) as f32 * dir) / s).clamp(36.0, 170.0);
            self.handle = (self.mouth.0 + (dir * distance * s).round() as i32,
                self.mouth.1 + ((input.cursor.1 - self.mouth.1) as f32).clamp(-48.0*s, 48.0*s).round() as i32);
            self.tension = (((distance - 80.0) / 90.0).clamp(0.0, 1.0) * 100.0).round() as u8;
            if input.button_down && self.tension >= 20 {
                self.effort = self.effort.saturating_add(input.dt_ms);
            }
            // Alternate bracing and giving a little. Bound every move to this work area.
            let brace = if (self.elapsed / 550) % 2 == 0 {28.0} else {8.0};
            let offset = dir * (self.tension as f32 * 0.20 - brace) * s;
            let target = (self.home.0 + offset.round() as i32).clamp(sx, sx + sw - input.win_size.0);
            let step = (100.0 * s * input.dt_ms.min(50) as f32 / 1000.0).round() as i32;
            out.movement = Some((input.win_pos.0 + (target-input.win_pos.0).clamp(-step, step),
                self.home.1.clamp(sy, sy + sh - input.win_size.1)));
            if !input.button_down || self.elapsed >= 12_000 {
                out.completed = self.effort >= 800 && self.elapsed >= 1500;
                self.phase = if out.completed {"tug_done"} else {"tug_ready"};
                self.elapsed = 0;
                self.tension = 0;
            }
        }
        self.was_down = input.button_down;
        self.align(input, out.movement.unwrap_or(input.win_pos));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(scale: f64) -> Input {
        Input { dt_ms:16, interactive:true, cursor:(-4000,-4000), win_pos:(-1300,300),
            win_size:((300.0*scale) as i32,(240.0*scale) as i32), scale_factor:scale,
            screen:(-1920,0,1920,1080), button_down:false, look_enabled:true,
            extra_animations:false, encounters_enabled:false, gravity:false, local_hour:12,
            sleep_frame:(crate::atlas::Row::Failed,2) }
    }
    #[test]
    fn real_grab_pull_release_completes_once_without_pet_drag() {
        for scale in [1.0,1.25,2.0] {
            for right in [false,true] {
                let mut i=input(scale);
                if !right {i.win_pos.0=-700;}
                let mut t=Tug::new(&i).unwrap();assert_eq!(t.right,right);
                i.cursor=t.handle;i.button_down=true;t.tick(&i);
                assert_eq!(t.phase,"tugging");
                let start=i.win_pos;let mut moved=false;
                for _ in 0..120 {
                    i.cursor=(t.mouth.0+(if right {150.0} else {-150.0}*scale) as i32,t.mouth.1);
                    let out=t.tick(&i);assert!(!out.completed);
                    if let Some(p)=out.movement {i.win_pos=p;moved|=p!=start;}
                    assert!(t.pointer_hot((-4000,-4000),scale));
                    assert!(i.win_pos.0>=i.screen.0&&i.win_pos.0+i.win_size.0<=0);
                }
                assert!(moved);assert!(t.tension>=20);
                i.button_down=false;assert!(t.tick(&i).completed);assert_eq!(t.phase,"tug_done");
                assert!(t.view().is_none());
                for _ in 0..120 {assert!(!t.tick(&i).completed);}
                assert_eq!(t.phase,"tug_ready");assert!(t.view().is_some());
            }
        }
    }
    #[test]
    fn builtin_bite_tip_tracks_each_pose_at_all_scales_and_directions() {
        for scale in [1.0,1.25,2.0] {for right in [false,true] {
            let mut i=input(scale);i.extra_animations=true;
            if !right {i.win_pos.0=-700;}
            let mut t=Tug::new(&i).unwrap();assert_eq!(t.right,right);
            i.cursor=t.handle;i.button_down=true;t.tick(&i);i.dt_ms=275;
            let mut seen=std::collections::BTreeSet::new();
            for _ in 0..12 {
                i.cursor=(t.mouth.0+(if right {150.0} else {-150.0}*scale) as i32,t.mouth.1);
                let step=t.tick(&i);if let Some(p)=step.movement {i.win_pos=p;}
                let col=t.frame();seen.insert(col);
                let (x,y)=if right {TUG_RIGHT[col]} else {TUG_LEFT[col]};
                assert_eq!(t.mouth,(i.win_pos.0+((PET_X as f32+x*0.75)*scale as f32).round() as i32,
                    i.win_pos.1+((PET_Y as f32+y*0.75)*scale as f32).round() as i32));
            }
            assert_eq!(seen,std::collections::BTreeSet::from([0,1,2,3]));
        }}
    }
    #[test]
    fn clicks_no_pull_and_suspension_do_not_complete_a_round() {
        for mode in [0,1,2,3] {
            let mut i=input(1.0);let mut t=Tug::new(&i).unwrap();
            i.cursor=t.handle;i.button_down=true;t.tick(&i);
            for _ in 0..110 {assert!(!t.tick(&i).completed);}
            match mode {0=>i.button_down=false,1=>i.interactive=false,2=>i.dt_ms=5000,_=>i.screen.2=100}
            assert!(!t.tick(&i).completed);
            assert_eq!(t.phase,if mode==0 {"tug_ready"} else {"off"});
        }
    }
    #[test]
    fn screen_edges_and_extreme_mouse_positions_keep_handle_on_screen() {
        for scale in [1.0,1.25,2.0] {
            let mut i=input(scale);i.win_pos=(i.screen.0,0);
            let mut t=Tug::new(&i).unwrap();i.cursor=t.handle;i.button_down=true;t.tick(&i);
            i.cursor=(i32::MAX/4,i32::MIN/4);
            for _ in 0..1000 {
                let out=t.tick(&i);if let Some(p)=out.movement {i.win_pos=p;}
                if let Some(v)=t.view() {assert!(v.handle.0>=i.screen.0&&v.handle.0<=0);assert!(v.handle.1>=0&&v.handle.1<=1080);}
            }
            assert_ne!(t.phase,"tugging"); // A held round is bounded even if release is lost.
        }
    }
}
