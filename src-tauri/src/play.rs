//! Desktop toys use physical coordinates, independent of the small pet window.
use crate::engine::{Input, PET_H, PET_W, PET_X, PET_Y};
use serde::Serialize;

// Atlas-pixel ball centres in the baked biting frames, used for release handoff.
const CARRY_RIGHT:[(f32,f32);8]=[(162.0,112.0),(162.0,112.0),(163.0,108.0),(163.0,109.0),(163.0,113.0),(164.0,100.0),(163.0,109.0),(163.0,110.0)];
const DISC_RIGHT:[(f32,f32);8]=[(153.0,122.2),(156.7,121.3),(156.8,120.9),(156.6,122.2),(155.5,117.9),(158.7,113.7),(157.0,112.3),(159.1,117.9)];
const DISC_LEFT:[(f32,f32);8]=[(33.8,120.1),(33.8,121.6),(32.3,124.5),(33.4,121.5),(34.4,123.2),(31.8,122.2),(33.3,127.1),(35.1,122.3)];
const CARRY_LEFT:[(f32,f32);8]=[(32.0,119.0),(32.0,120.0),(31.0,120.0),(31.0,121.0),(32.0,119.0),(31.0,117.0),(31.0,120.0),(31.0,118.0)];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayView {
    pub tug: Option<crate::tug::TugView>,
    pub tug_rounds: u32,
    pub phase: &'static str,
    pub ball: Option<(i32, i32)>,
    pub catches: u32,
    pub streak: u32,
    pub toy: &'static str,
    pub last_catch: &'static str,
    pub style: &'static str,
}

pub struct Play {
    tug: Option<crate::tug::Tug>,
    tug_rounds: u32,
    phase: &'static str,
    ball: Option<(f32, f32)>,
    velocity: (f32, f32),
    home: (i32, i32),
    last_cursor: (i32, i32),
    held_offset: (f32, f32),
    was_down: bool,
    elapsed: u64,
    pub catches: u32,
    release_ms: u64,
    release_from: (f32,f32),
    carry_right: bool,
    playful: bool,
    teased: bool,
    tease_ms: u64,
    streak: u32,
    toy: &'static str,
    last_catch: &'static str,
    catch_ms:u64,
    catch_home:(i32,i32),
    style: &'static str,
}

pub struct PlayStep {
    pub movement: Option<(i32, i32)>,
    pub direction: Option<bool>,
    pub completed: bool,
}

impl Default for Play {
    fn default() -> Self {
        Self { tug: None, tug_rounds: 0, phase: "off", ball: None, velocity: (0.0, 0.0), home: (0, 0),
            last_cursor: (0, 0), held_offset: (0.0, 0.0), was_down: false, elapsed: 0, catches: 0, release_ms: 0, release_from: (0.0,0.0), carry_right: true, playful:true, teased:false, tease_ms:0, streak:0, toy:"ball", last_catch:"none", catch_ms:0, catch_home:(0,0), style:"normal" }
    }
}

impl Play {
    pub fn set_playful(&mut self,enabled:bool) {self.playful=enabled;if !enabled {self.drop_ball();}}
    pub fn drop_ball(&mut self) {if self.phase=="teasing" {self.phase="returning";self.teased=true;}}
    pub fn active(&self) -> bool { self.phase != "off" }
    pub fn pointer_hot(&self, cursor: (i32,i32), scale: f64) -> bool {
        if let Some(tug)=&self.tug {return tug.pointer_hot(cursor,scale);}
        self.ball.is_some_and(|(x,y)| (cursor.0 as f32-x).hypot(cursor.1 as f32-y) <= 22.0*scale as f32)
    }
    pub fn view(&self) -> PlayView {
        PlayView { tug:self.tug.as_ref().and_then(|t|t.view()), tug_rounds:self.tug_rounds, phase: self.phase, ball: self.ball.map(|(x,y)| (x.round() as i32,y.round() as i32)), catches: self.catches, streak:self.streak, style:self.style, toy:self.toy, last_catch:self.last_catch }
    }
    pub fn tug_frame(&self) -> Option<usize> {
        self.tug.as_ref().filter(|t|t.direction().is_some()).map(|t|t.frame())
    }
    pub fn cancel(&mut self) { self.tug=None; self.phase = "off"; self.ball = None; self.velocity = (0.0,0.0); self.streak=0; }
    pub fn roll(&mut self,input:&Input) {
        if self.phase!="returned" || self.toy=="frisbee" {return;}
        if let Some((x,_))=self.ball {
            let dir=if input.cursor.0 as f32>=x {1.0} else {-1.0};
            self.velocity=(dir*200.0*input.scale_factor as f32,0.0);self.phase="rolling";self.elapsed=0;
        }
    }
    pub fn chase_speed(&self)->f32 {if self.toy=="frisbee" {return 560.0;}match self.style {"near"=>180.0,"far"=>390.0,_=>310.0}}
    pub fn start(&mut self,input:&Input,throw:bool) {
        let origin=if throw && self.toy=="ball" && matches!(self.phase,"ready"|"returned") {self.ball} else {None};
        if self.toy!="ball" {self.streak=0;}
        self.toy="ball";self.launch(input,throw,origin);
    }
    pub fn start_tug(&mut self,input:&Input) {
        self.cancel();self.toy="rope";
        self.tug=crate::tug::Tug::new(input);
        self.phase=self.tug.as_ref().map_or("off",|t|t.phase);
        self.ball=self.tug.as_ref().and_then(|t|t.view()).map(|v|(v.handle.0 as f32,v.handle.1 as f32));
    }
    pub fn start_frisbee(&mut self,input:&Input,throw:bool) {
        // A button throw reuses the visible disc, including its exact release spot.
        // Capture this before changing toys so a returned ball never becomes a disc origin.
        let origin=if throw && self.toy=="frisbee" && matches!(self.phase,"ready"|"returned") {self.ball} else {None};
        if self.toy!="frisbee" {self.streak=0;}
        self.toy="frisbee";self.launch(input,throw,origin);
    }
    fn launch(&mut self, input: &Input, throw: bool, origin: Option<(f32,f32)>) {
        if !input.interactive { return; }
        self.tug=None;
        if self.phase=="off" || !throw {self.streak=0;}
        self.last_catch="none";self.catch_ms=0;
        self.style=if throw {"far"} else {"normal"};
        let s = input.scale_factor as f32;
        let (sx,sy,sw,sh) = input.screen;
        if sw < input.win_size.0 || sh < input.win_size.1 { return; }
        self.home = (input.win_pos.0.clamp(sx,sx+sw-input.win_size.0), input.win_pos.1.clamp(sy,sy+sh-input.win_size.1));
        let center = self.home.0 as f32 + (PET_X + PET_W/2) as f32*s;
        let dir = if origin.map_or(center,|p|p.0) < (sx+sw/2) as f32 { 1.0 } else { -1.0 };
        let radius = (if self.toy=="frisbee" {18.0} else {14.0})*s;
        let (x,y)=origin.unwrap_or((center+dir*100.0*s,self.home.1 as f32+(PET_Y+PET_H-24) as f32*s));
        self.ball = Some((x.clamp(sx as f32+radius,(sx+sw) as f32-radius),
            y.clamp(sy as f32+radius,(sy+sh) as f32-radius)));
        self.velocity = if throw { if self.toy=="frisbee" {(dir*430.0*s,-130.0*s)} else {(dir*330.0*s,-380.0*s)} } else { (0.0,0.0) };
        self.phase = if throw { "chasing" } else { "ready" };
        self.elapsed = 0;
        self.teased = false; self.tease_ms = 0;
        self.was_down = input.button_down;
        self.last_cursor = input.cursor;
    }
    pub fn invite(&mut self,input:&Input) {
        self.start(input,false);
        if let Some((x,y))=self.ball.as_mut() {
            let center=input.win_pos.0 as f32+(PET_X+PET_W/2) as f32*input.scale_factor as f32;
            let dir=if *x>=center {1.0} else {-1.0};
            *x=center+dir*68.0*input.scale_factor as f32;
            *y=input.win_pos.1 as f32+(PET_Y+PET_H-14) as f32*input.scale_factor as f32;
        }
    }
    pub fn nudge(&mut self,delta:(i32,i32)) {
        if self.phase=="ready" { if let Some((x,y))=self.ball.as_mut() { *x+=delta.0 as f32;*y+=delta.1 as f32; } }
    }
    /// Track the carried ball for legacy overlay and builtin release handoff.
    pub fn align_carried_ball(&mut self, input:&Input, position:(i32,i32), row:crate::atlas::Row, col:usize) {
        if let Some(tug)=self.tug.as_mut() {
            tug.align(input,position);
            self.ball=tug.view().map(|v|(v.handle.0 as f32,v.handle.1 as f32));
            return;
        }
        if !matches!(self.phase,"returning" | "teasing" | "catching") { return; }
        let anchor=match row {
            crate::atlas::Row::DiscRight => DISC_RIGHT[col.min(7)],
            crate::atlas::Row::DiscLeft => DISC_LEFT[col.min(7)],
            crate::atlas::Row::CarryRight => CARRY_RIGHT[col.min(7)],
            crate::atlas::Row::CarryLeft => CARRY_LEFT[col.min(7)],
            crate::atlas::Row::RunRight => (182.0,94.0),
            crate::atlas::Row::RunLeft => (10.0,94.0),
            _ => return,
        };
        let scale=input.scale_factor as f32;
        self.ball=Some((position.0 as f32+(PET_X as f32+anchor.0*0.75)*scale,
            position.1 as f32+(PET_Y as f32+anchor.1*0.75)*scale));
    }
    pub fn tick(&mut self, input: &Input) -> PlayStep {
        let mut out = PlayStep { movement: None, direction: None, completed: false };
        if let Some(tug)=self.tug.as_mut() {
            let step=tug.tick(input);
            self.phase=tug.phase;
            self.ball=tug.view().map(|v|(v.handle.0 as f32,v.handle.1 as f32));
            if step.completed {self.tug_rounds=self.tug_rounds.saturating_add(1);}
            return PlayStep {movement:step.movement,direction:tug.direction(),completed:step.completed};
        }
        if !self.active() { self.was_down = input.button_down; return out; }
        if !input.interactive { self.cancel(); return out; }
        let s = input.scale_factor as f32;
        let dt = input.dt_ms.min(50) as f32/1000.0;
        self.elapsed = self.elapsed.saturating_add(input.dt_ms);
        if self.elapsed > 30_000 && !matches!(self.phase,"ready" | "returned") { self.cancel(); return out; }
        let radius = if self.toy=="frisbee" {18.0*s} else {14.0*s};
        let (sx,sy,sw,sh) = input.screen;
        if sw < input.win_size.0 || sh < input.win_size.1 { self.cancel(); return out; }
        self.home.0 = self.home.0.clamp(sx,sx+sw-input.win_size.0);
        self.home.1 = self.home.1.clamp(sy,sy+sh-input.win_size.1);
        let (mut x,mut y) = self.ball.unwrap();
        if matches!(self.phase, "ready" | "chasing" | "returned" | "rolling") && input.button_down && !self.was_down
            && (input.cursor.0 as f32-x).hypot(input.cursor.1 as f32-y) <= radius+4.0*s {
            self.phase = "held";
            self.held_offset = (x-input.cursor.0 as f32,y-input.cursor.1 as f32);
            self.velocity = (0.0,0.0);
            self.home = input.win_pos;
            self.teased = false; self.tease_ms = 0;
            self.elapsed = 0;
        }
        if self.phase == "held" {
            if input.button_down {
                x = input.cursor.0 as f32+self.held_offset.0;
                y = input.cursor.1 as f32+self.held_offset.1;
                if dt > 0.0 {
                    self.velocity = (((input.cursor.0-self.last_cursor.0) as f32/dt).clamp(-900.0*s,900.0*s),
                        ((input.cursor.1-self.last_cursor.1) as f32/dt).clamp(-900.0*s,900.0*s));
                }
            } else {
                // The cursor may already have moved after mouse-up by this
                // polling tick. Release the last actually held toy position.
                self.phase = "chasing";
                let distance=(x-(self.home.0 as f32+(PET_X+PET_W/2) as f32*s)).hypot(y-(self.home.1 as f32+(PET_Y+PET_H-14) as f32*s))/s;
                self.style=if distance<140.0 {"near"} else if distance>300.0 {"far"} else {"normal"};
                self.elapsed = 0;
            }
        }
        x = x.clamp(sx as f32+radius,(sx+sw) as f32-radius);
        y = y.clamp(sy as f32+radius,(sy+sh) as f32-radius);
        if self.phase == "rolling" {
            x+=self.velocity.0*dt;
            self.velocity.0*=0.97_f32.powf(dt*60.0);
            if self.elapsed>=2500 || x<=sx as f32+radius || x>=(sx+sw) as f32-radius || self.velocity.0.abs()<6.0*s {
                self.phase="returned";self.velocity=(0.0,0.0);
            }
        }
        if self.phase == "chasing" {
            self.velocity.1 += (if self.toy=="frisbee" {240.0} else {1100.0})*s*dt;
            if self.toy=="frisbee" {self.velocity.0*=0.998_f32.powf(dt*60.0);}
            x += self.velocity.0*dt;
            y += self.velocity.1*dt;
            if x < sx as f32+radius || x > (sx+sw) as f32-radius {
                x = x.clamp(sx as f32+radius,(sx+sw) as f32-radius);
                self.velocity.0 *= -0.6;
            }
            if y < sy as f32+radius { y = sy as f32+radius; self.velocity.1 = self.velocity.1.abs()*0.6; }
            if y >= (sy+sh) as f32-radius {
                y = (sy+sh) as f32-radius;
                self.velocity.1 = if self.toy=="frisbee" {0.0} else if self.velocity.1.abs() > 70.0*s { -self.velocity.1*0.45 } else { 0.0 };
                self.velocity.0 *= 0.88_f32.powf(dt*60.0);
            }
            let tx = (x-(PET_X+PET_W/2) as f32*s).round() as i32;
            let ty = (y-(if self.toy=="frisbee" {PET_Y+84} else {PET_Y+PET_H-14}) as f32*s).round() as i32;
            let target = (tx.clamp(sx,sx+sw-input.win_size.0),ty.clamp(sy,sy+sh-input.win_size.1));
            let dx = target.0-input.win_pos.0;
            let dy = target.1-input.win_pos.1;
            let distance = (dx as f32).hypot(dy as f32);
            if dx!=0 {self.carry_right=dx>0;}
            if distance <= 12.0*s && self.velocity.1.abs() < (if self.toy=="frisbee" {400.0} else {120.0})*s {
                let air=self.toy=="frisbee" && y<(sy+sh) as f32-radius-2.0*s;
                self.last_catch=if air {"air"} else {"ground"};
                self.phase = if air {"catching"} else {"returning"};
                self.catch_ms=0;self.catch_home=input.win_pos;
            } else {
                out.movement = Some(step_toward(input.win_pos,target,self.chase_speed()*s*dt));
                out.direction = Some(dx >= 0);
            }
        }
        if self.phase=="catching" {
            self.catch_ms=self.catch_ms.saturating_add(input.dt_ms);
            let t=(self.catch_ms as f32/450.0).min(1.0);
            let pos=(self.catch_home.0,(self.catch_home.1 as f32-(std::f32::consts::PI*t).sin()*18.0*s).round() as i32);
            out.movement=Some((pos.0.clamp(sx,sx+sw-input.win_size.0),pos.1.clamp(sy,sy+sh-input.win_size.1)));
            out.direction=Some(self.carry_right);
            if self.catch_ms>=450 {self.phase="returning";}
        }
        if self.phase == "teasing" {
            self.tease_ms = self.tease_ms.saturating_add(input.dt_ms);
            let near = (input.cursor.0 as f32-(input.win_pos.0 as f32+(PET_X+PET_W/2) as f32*s)).hypot(input.cursor.1 as f32-(input.win_pos.1 as f32+(PET_Y+PET_H/2) as f32*s)) < 85.0*s;
            if self.tease_ms>=3500 || near || !self.playful {self.phase="returning";}
            else {
                let offset = if self.carry_right {-56.0*s} else {56.0*s};
                let target=((self.home.0 as f32+offset).round() as i32,self.home.1);
                let target=(target.0.clamp(sx,sx+sw-input.win_size.0),target.1);
                out.movement=Some(step_toward(input.win_pos,target,110.0*s*dt));
                out.direction=Some(self.carry_right);
            }
        }
        if self.phase == "returning" {
            let dx = self.home.0-input.win_pos.0;
            let dy = self.home.1-input.win_pos.1;
            let position = step_toward(input.win_pos,self.home,(if self.style=="near" {150.0} else {250.0})*s*dt);
            out.movement = Some(position);
            if dx != 0 { self.carry_right = dx > 0; }
            out.direction = Some(self.carry_right);
            // Fallback placement; the engine aligns the toy to the selected mouth frame.
            x = position.0 as f32 + (if self.carry_right { PET_X+PET_W-18 } else { PET_X+18 }) as f32*s;
            y = position.1 as f32 + (PET_Y+PET_H/3) as f32*s;
            if (dx as f32).hypot(dy as f32) <= 5.0*s {
                out.movement = Some(self.home);
                out.direction = None;
                if self.toy=="ball" && input.extra_animations && self.playful && !self.teased && (self.streak+1)%3==0 {
                    self.phase="teasing";self.teased=true;self.tease_ms=0;
                    out.direction=Some(self.carry_right);
                } else if input.extra_animations {
                    self.phase = "releasing";
                    self.release_ms = 0;
                    // The standing release pose has a higher muzzle than the
                    // running carry pose. Start at its open jaws, then move
                    // outside the front paws before the toy starts falling.
                    let mouth_x = if self.carry_right {180.0} else {14.0};
                    self.release_from = (self.home.0 as f32+(PET_X as f32+mouth_x*0.75)*s,
                        self.home.1 as f32+(PET_Y as f32+105.0*0.75)*s);
                    x = self.release_from.0; y = self.release_from.1;
                    out.direction = Some(self.carry_right);
                } else {
                    self.phase = "returned";
                    self.elapsed = 0;
                    self.catches = self.catches.saturating_add(1);
                    self.streak=self.streak.saturating_add(1);
                    out.completed = true;
                    y = self.home.1 as f32+(PET_Y+PET_H-14) as f32*s;
                }
            }
        }
        if self.phase == "releasing" {
            self.release_ms = self.release_ms.saturating_add(input.dt_ms);
            out.movement = Some(self.home);
            out.direction = Some(self.carry_right);
            let outward = (self.release_ms as f32/80.0).min(1.0);
            let t = (self.release_ms.saturating_sub(80) as f32/320.0).min(1.0);
            let floor = self.home.1 as f32+(PET_Y+PET_H-14) as f32*s;
            x = self.release_from.0 + if self.carry_right {34.0*s*outward} else {-34.0*s*outward};
            y = self.release_from.1+(floor-self.release_from.1)*t*t;
            if self.release_ms >= 400 {
                self.phase = "returned"; self.elapsed = 0;
                self.catches = self.catches.saturating_add(1);
                    self.streak=self.streak.saturating_add(1);
                out.direction = None; out.completed = true;
            }
        }
        self.ball = Some((x.clamp(sx as f32+radius,(sx+sw) as f32-radius),y.clamp(sy as f32+radius,(sy+sh) as f32-radius)));
        self.last_cursor = input.cursor;
        self.was_down = input.button_down;
        out
    }
}

fn step_toward(from: (i32,i32), to: (i32,i32), step: f32) -> (i32,i32) {
    let dx = (to.0-from.0) as f32;
    let dy = (to.1-from.1) as f32;
    let distance = dx.hypot(dy);
    if distance <= step || distance == 0.0 { return to; }
    (from.0+(dx/distance*step).round() as i32,from.1+(dy/distance*step).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(scale: f64) -> Input {
        Input { dt_ms:16, interactive:true, cursor:(-4000,-4000), win_pos:(-1300,300),
            win_size:((300.0*scale) as i32,(240.0*scale) as i32),scale_factor:scale,
            screen:(-1920,0,1920,1080), button_down:false, look_enabled:true,
            extra_animations:true, encounters_enabled:false, gravity:false,local_hour:12,sleep_frame:(crate::atlas::Row::Failed,2) }
    }
    #[test]
    fn real_release_distance_changes_pace_and_rolling_remains_grabbable() {
        for scale in [1.0,1.25,2.0] {
            let mut i=input(scale);
            let mut speeds=Vec::new();
            for distance in [60.0,400.0] {
                let mut p=Play::default();p.start(&i,false);p.phase="held";p.held_offset=(0.0,0.0);
                i.cursor=(i.win_pos.0+((PET_X+PET_W/2) as f64*scale+distance*scale) as i32,i.win_pos.1+((PET_Y+PET_H-14) as f64*scale) as i32);
                i.button_down=true;p.tick(&i);i.button_down=false;
                p.tick(&i);speeds.push(p.chase_speed());
                assert_eq!(p.view().style,if distance<140.0 {"near"} else {"far"});
            }
            assert!(speeds[1]>speeds[0]*2.0);
            let mut p=Play::default();p.start(&i,false);p.phase="returned";p.catches=2;p.streak=2;
            let before=p.view().ball.unwrap();p.roll(&i);p.tick(&i);
            assert_eq!(p.view().phase,"rolling");assert_ne!(p.view().ball.unwrap().0,before.0);
            i.cursor=p.view().ball.unwrap();i.button_down=true;p.tick(&i);assert_eq!(p.view().phase,"held");
            assert_eq!(p.catches,2);assert_eq!(p.streak,2);
        }
    }
    #[test]
    fn playful_return_is_bounded_optional_and_releases_once() {
        for scale in [1.0,1.25,2.0] {
            for mode in [0,1,2,3] {
                let mut i=input(scale);let mut p=Play::default();p.start(&i,false);p.phase="returning";p.catches=2;p.streak=2;
                if mode==3 {i.extra_animations=false;}
                p.tick(&i);
                if mode==3 {assert_eq!(p.view().phase,"returned");continue;}
                assert_eq!(p.view().phase,"teasing");assert_eq!(p.catches,2);
                if mode==1 {p.drop_ball();} else if mode==2 {p.set_playful(false);}
                let mut completions=0;
                for _ in 0..400 {
                    let step=p.tick(&i);completions+=usize::from(step.completed);
                    if let Some(pos)=step.movement {i.win_pos=pos;}
                    assert!((i.win_pos.0-p.home.0).abs()<= (56.0*scale).round() as i32);
                    if p.view().phase=="returned" {break;}
                }
                assert_eq!(p.view().phase,"returned");assert_eq!(completions,1);assert_eq!(p.catches,3);
            }
        }
    }
    #[test]
    fn mouse_up_does_not_teleport_the_toy_to_a_later_cursor_position() {
        for scale in [1.0,1.25,2.0] {
            for toy in ["ball","frisbee"] {
                let mut i=input(scale);let mut p=Play::default();p.start(&i,false);p.toy=toy;
                i.cursor=p.view().ball.unwrap();i.button_down=true;p.tick(&i);
                assert_eq!(p.phase,"held");
                // A stationary hold has no horizontal release velocity.
                p.tick(&i);let held=p.view().ball.unwrap();
                i.button_down=false;i.cursor=(-1800,30);p.tick(&i);
                let released=p.view().ball.unwrap();assert_eq!(p.phase,"chasing");
                assert_eq!(released.0,held.0);assert!((released.1-held.1).abs()<=2);
            }
        }
    }
    #[test]
    fn returned_ball_persists_and_accepts_another_throw() {
        let mut i=input(1.0);let mut p=Play::default();p.start(&i,false);
        p.phase="returned";
        for _ in 0..2500 {p.tick(&i);}
        assert_eq!(p.view().phase,"returned");
        i.cursor=p.view().ball.unwrap();i.button_down=true;p.tick(&i);
        assert_eq!(p.view().phase,"held");
        i.button_down=false;p.tick(&i);
        assert_eq!(p.view().phase,"chasing");
        p.cancel();assert!(p.view().ball.is_none());
    }
    #[test]
    fn reaching_home_exactly_preserves_left_release_direction() {
        for scale in [1.0,2.0] {
            let mut i=input(scale);i.dt_ms=50;
            let mut p=Play::default();p.start(&i,false);p.phase="returning";p.carry_right=false;
            i.win_pos.0+=20;
            for _ in 0..10 {
                let out=p.tick(&i);if let Some(pos)=out.movement {i.win_pos=pos;}
                if p.view().phase=="releasing" {assert_eq!(out.direction,Some(false));break;}
            }
            assert_eq!(p.view().phase,"releasing");
        }
    }
    #[test]
    fn builtin_release_falls_to_feet_and_counts_once() {
        for scale in [1.0,1.25,2.0] {
            let mut i=input(scale);let mut p=Play::default();p.start(&i,false);
            p.phase="returning";p.ball=Some((i.win_pos.0 as f32+170.0*scale as f32,i.win_pos.1 as f32+170.0*scale as f32));
            let first=p.tick(&i);assert!(!first.completed);assert_eq!(p.view().phase,"releasing");
            let mut y=p.view().ball.unwrap().1;
            let mut completed=0;
            for _ in 0..40 {
                let step=p.tick(&i);completed+=usize::from(step.completed);
                let next=p.view().ball.unwrap().1;assert!(next>=y);y=next;
            }
            assert_eq!(completed,1);assert_eq!(p.catches,1);assert_eq!(p.view().phase,"returned");
            assert_eq!(y,i.win_pos.1+((PET_Y+PET_H-14) as f64*scale).round() as i32);
            i.button_down=true;i.cursor=p.view().ball.unwrap();p.tick(&i);assert_eq!(p.view().phase,"held");
        }
    }
    #[test]
    fn release_clears_the_muzzle_and_paws_before_falling_for_both_toys() {
        for scale in [1.0,1.25,2.0] {
            for toy in ["ball","frisbee"] {
                for right in [false,true] {
                    let mut i=input(scale);
                    let mut p=Play::default();p.start(&i,false);
                    p.toy=toy;p.phase="returning";p.carry_right=right;
                    // The old carry anchor is deliberately below the open jaws.
                    p.ball=Some((i.win_pos.0 as f32+150.0*scale as f32,i.win_pos.1 as f32+190.0*scale as f32));
                    let mut completed=0;let mut falling=false;
                    for _ in 0..30 {
                        let step=p.tick(&i);completed+=usize::from(step.completed);
                        if let Some(position)=step.movement {i.win_pos=position;}
                        let (x,y)=p.view().ball.unwrap();
                        let logical=((x-i.win_pos.0) as f64/scale,(y-i.win_pos.1) as f64/scale);
                        assert!((35.0..265.0).contains(&logical.0));
                        if p.phase=="releasing" && p.release_ms<=80 {
                            assert!((logical.1-(PET_Y as f64+105.0*0.75)).abs()<=0.6);
                        }
                        if logical.1>180.0 {
                            falling=true;
                            // Disc half-width 18 also bounds the smaller ball.
                            assert!(if right {logical.0-18.0>222.0} else {logical.0+18.0<78.0},"toy crossed the pet's paws");
                        }
                    }
                    assert!(falling);assert_eq!(completed,1);assert_eq!(p.catches,1);
                    assert_eq!(p.phase,"returned");
                }
            }
        }
    }
    #[test]
    fn carry_alignment_tracks_each_frame_at_multiple_scales() {
        for scale in [1.0,1.25,2.0] {
            let i=input(scale);let mut p=Play::default();p.phase="returning";
            let mut heights=std::collections::HashSet::new();
            for row in [crate::atlas::Row::CarryRight,crate::atlas::Row::CarryLeft] {
                for col in 0..8 {
                    p.align_carried_ball(&i,i.win_pos,row,col);
                    let (x,y)=p.view().ball.unwrap();
                    assert!(y>i.win_pos.1+(110.0*scale) as i32 && y<i.win_pos.1+(200.0*scale) as i32);
                    assert!(x>i.win_pos.0 && x<i.win_pos.0+i.win_size.0);
                    heights.insert(y);
                }
            }
            assert!(heights.len()>3,"mouth anchors must follow gait");
            p.phase="ready";let before=p.view().ball;
            p.align_carried_ball(&i,(0,0),crate::atlas::Row::CarryRight,0);
            assert_eq!(p.view().ball,before);
        }
    }
    #[test]
    fn fetch_returns_once_to_origin_at_multiple_scales() {
        for scale in [1.0,1.25,2.0] {
            let mut p=Play::default(); let mut i=input(scale); let origin=i.win_pos;
            p.start(&i,true); let mut completed=0; let mut carried=false;
            for _ in 0..2000 {
                let step=p.tick(&i);
                if let Some(pos)=step.movement { i.win_pos=pos; }
                carried |= p.view().phase=="returning";
                completed+=u32::from(step.completed);
                if p.view().phase=="returned" { break; }
            }
            assert!(carried,"scale {scale}"); assert_eq!(completed,1);
            assert_eq!(i.win_pos,origin); assert_eq!(p.catches,1);
            assert!(!p.tick(&i).completed);
        }
    }
    #[test]
    fn frisbee_glides_catches_and_returns_once_at_each_scale() {
        for scale in [1.0,1.25,2.0] {for builtin in [false,true] {
            let mut i=input(scale);i.extra_animations=builtin;let mut p=Play::default();p.start_frisbee(&i,true);
            let start=p.ball.unwrap();i.dt_ms=100;p.tick(&i);let after=p.ball.unwrap();
            assert!(after.0!=start.0 && (after.1-start.1).abs()<15.0*scale as f32);
            i.dt_ms=16;let mut completed=0;let mut phases=std::collections::BTreeSet::new();
            for _ in 0..3000 {phases.insert(p.phase);let step=p.tick(&i);completed+=usize::from(step.completed);if let Some(pos)=step.movement {i.win_pos=pos;}if p.phase=="returned" {break;}}
            assert_eq!(p.phase,"returned");assert_eq!(completed,1);assert_eq!(p.catches,1);assert_eq!(p.toy,"frisbee");assert!(phases.contains("catching"));assert_eq!(p.last_catch,"air");
            i.dt_ms=32_000;p.tick(&i);assert_eq!(p.phase,"returned");
            i.dt_ms=16;i.cursor=p.view().ball.unwrap();i.button_down=true;p.tick(&i);assert_eq!(p.phase,"held");
            i.button_down=false;p.tick(&i);assert_eq!(p.phase,"chasing");
            i.interactive=false;p.tick(&i);assert_eq!(p.phase,"off");assert!(p.ball.is_none());
        }}
    }
    #[test]
    fn ball_button_reuses_ready_and_returned_locations_without_reusing_other_toys() {
        for scale in [1.0,1.25,2.0] {
            let i=input(scale);let mut p=Play::default();p.start(&i,false);
            let ready=p.ball;p.start(&i,true);assert_eq!(p.ball,ready);
            p.phase="returned";p.ball=Some((-1500.0,800.0));p.catches=2;
            p.start(&i,true);assert_eq!(p.ball,Some((-1500.0,800.0)));assert_eq!(p.catches,2);
            p.toy="frisbee";p.phase="returned";p.ball=Some((-1800.0,800.0));
            p.start(&i,true);assert_ne!(p.ball,Some((-1800.0,800.0)));
        }
    }
    #[test]
    fn button_rethrows_disc_from_its_return_spot_and_counts_each_round_once() {
        for scale in [1.0,1.25,2.0] {for builtin in [false,true] {
            let mut i=input(scale);i.extra_animations=builtin;
            let mut p=Play::default();p.start_frisbee(&i,true);
            for round in 1..=3 {
                let mut completed=0;
                for _ in 0..3000 {
                    let step=p.tick(&i);completed+=usize::from(step.completed);
                    if let Some(pos)=step.movement {i.win_pos=pos;}
                    if p.phase=="returned" {break;}
                }
                assert_eq!(p.phase,"returned");assert_eq!(completed,1);
                assert_eq!(p.catches,round);assert_eq!(p.streak,round);
                let returned=p.ball.unwrap();
                p.start_frisbee(&i,true);
                assert_eq!(p.ball,Some(returned),"button must not teleport the returned disc");
                assert_eq!(p.phase,"chasing");assert_eq!(p.catches,round);assert_eq!(p.streak,round);
                assert_eq!(p.last_catch,"none");assert_eq!(p.home,i.win_pos);
            }
        }}
    }
    #[test]
    fn button_uses_ready_disc_but_not_a_different_toy_or_stale_screen_position() {
        let mut i=input(1.0);let mut p=Play::default();p.start_frisbee(&i,false);
        let ready=p.ball;p.start_frisbee(&i,true);assert_eq!(p.ball,ready);
        p.start(&i,false);p.phase="returned";p.ball=Some((-1800.0,800.0));
        p.start_frisbee(&i,true);assert_ne!(p.ball,Some((-1800.0,800.0)));
        p.phase="returned";p.ball=Some((-1800.0,800.0));
        i.screen=(0,0,1280,720);i.win_pos=(400,200);
        p.start_frisbee(&i,true);assert_eq!(p.ball,Some((18.0,702.0)));
        assert!(p.velocity.0>0.0,"launch toward available room on the current screen");
    }
    #[test]
    fn dragging_ball_outside_screen_and_hiding_cleans_up() {
        let mut p=Play::default(); let mut i=input(1.5); p.start(&i,false);
        i.cursor=p.view().ball.unwrap(); i.button_down=true; p.tick(&i);
        assert_eq!(p.view().phase,"held"); i.cursor=(9999,-9999); p.tick(&i);
        let ball=p.view().ball.unwrap(); assert!(ball.0<0 && ball.1>=0);
        i.button_down=false; p.tick(&i); assert_eq!(p.view().phase,"chasing");
        i.interactive=false; let out=p.tick(&i);
        assert!(out.movement.is_none()); assert_eq!(p.view().phase,"off"); assert!(p.view().ball.is_none());
    }
    #[test]
    fn display_removal_clamps_return_point_and_long_pause_expires_play() {
        let mut p=Play::default(); let mut i=input(1.0); p.start(&i,true);
        i.screen=(0,0,1280,720); i.win_pos=(400,200);
        for _ in 0..1500 { let out=p.tick(&i); if let Some(pos)=out.movement { i.win_pos=pos; assert!(pos.0>=0 && pos.0<=980 && pos.1>=0 && pos.1<=480); } }
        assert_eq!(p.view().phase,"returned");
        p.start(&i,true);i.dt_ms=60_000; p.tick(&i); assert_eq!(p.view().phase,"off");
    }
}
