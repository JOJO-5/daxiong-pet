//! Desktop toys use physical coordinates, independent of the small pet window.
use crate::engine::{Input, PET_H, PET_W, PET_X, PET_Y};
use serde::Serialize;

// Atlas-pixel ball centres in the baked biting frames, used for release handoff.
const CARRY_RIGHT:[(f32,f32);8]=[(162.0,112.0),(162.0,112.0),(163.0,108.0),(163.0,109.0),(163.0,113.0),(164.0,100.0),(163.0,109.0),(163.0,110.0)];
const CARRY_LEFT:[(f32,f32);8]=[(32.0,119.0),(32.0,120.0),(31.0,120.0),(31.0,121.0),(32.0,119.0),(31.0,117.0),(31.0,120.0),(31.0,118.0)];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayView {
    pub phase: &'static str,
    pub ball: Option<(i32, i32)>,
    pub catches: u32,
    pub streak: u32,
    pub style: &'static str,
}

pub struct Play {
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
    style: &'static str,
}

pub struct PlayStep {
    pub movement: Option<(i32, i32)>,
    pub direction: Option<bool>,
    pub completed: bool,
}

impl Default for Play {
    fn default() -> Self {
        Self { phase: "off", ball: None, velocity: (0.0, 0.0), home: (0, 0),
            last_cursor: (0, 0), held_offset: (0.0, 0.0), was_down: false, elapsed: 0, catches: 0, release_ms: 0, release_from: (0.0,0.0), carry_right: true, playful:true, teased:false, tease_ms:0, streak:0, style:"normal" }
    }
}

impl Play {
    pub fn set_playful(&mut self,enabled:bool) {self.playful=enabled;if !enabled {self.drop_ball();}}
    pub fn drop_ball(&mut self) {if self.phase=="teasing" {self.phase="returning";self.teased=true;}}
    pub fn active(&self) -> bool { self.phase != "off" }
    pub fn pointer_hot(&self, cursor: (i32,i32), scale: f64) -> bool {
        self.ball.is_some_and(|(x,y)| (cursor.0 as f32-x).hypot(cursor.1 as f32-y) <= 18.0*scale as f32)
    }
    pub fn view(&self) -> PlayView {
        PlayView { phase: self.phase, ball: self.ball.map(|(x,y)| (x.round() as i32,y.round() as i32)), catches: self.catches, streak:self.streak, style:self.style }
    }
    pub fn cancel(&mut self) { self.phase = "off"; self.ball = None; self.velocity = (0.0,0.0); self.streak=0; }
    pub fn roll(&mut self,input:&Input) {
        if self.phase!="returned" {return;}
        if let Some((x,_))=self.ball {
            let dir=if input.cursor.0 as f32>=x {1.0} else {-1.0};
            self.velocity=(dir*200.0*input.scale_factor as f32,0.0);self.phase="rolling";self.elapsed=0;
        }
    }
    pub fn chase_speed(&self)->f32 {match self.style {"near"=>180.0,"far"=>390.0,_=>310.0}}
    pub fn start(&mut self, input: &Input, throw: bool) {
        if !input.interactive { return; }
        if self.phase=="off" || !throw {self.streak=0;}
        self.style=if throw {"far"} else {"normal"};
        let s = input.scale_factor as f32;
        let (sx,sy,sw,sh) = input.screen;
        if sw < input.win_size.0 || sh < input.win_size.1 { return; }
        self.home = (input.win_pos.0.clamp(sx,sx+sw-input.win_size.0), input.win_pos.1.clamp(sy,sy+sh-input.win_size.1));
        let center = self.home.0 as f32 + (PET_X + PET_W/2) as f32*s;
        let dir = if center < (sx+sw/2) as f32 { 1.0 } else { -1.0 };
        let radius = 14.0*s;
        self.ball = Some(((center+dir*100.0*s).clamp(sx as f32+radius,(sx+sw) as f32-radius),
            (self.home.1 as f32+(PET_Y+PET_H-24) as f32*s).clamp(sy as f32+radius,(sy+sh) as f32-radius)));
        self.velocity = if throw { (dir*330.0*s,-380.0*s) } else { (0.0,0.0) };
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
        if !matches!(self.phase,"returning" | "teasing") { return; }
        let anchor=match row {
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
        if !self.active() { self.was_down = input.button_down; return out; }
        if !input.interactive { self.cancel(); return out; }
        let s = input.scale_factor as f32;
        let dt = input.dt_ms.min(50) as f32/1000.0;
        self.elapsed = self.elapsed.saturating_add(input.dt_ms);
        if self.elapsed > 30_000 && !matches!(self.phase,"ready" | "returned") { self.cancel(); return out; }
        let radius = 14.0*s;
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
            x = input.cursor.0 as f32+self.held_offset.0;
            y = input.cursor.1 as f32+self.held_offset.1;
            if input.button_down {
                if dt > 0.0 {
                    self.velocity = (((input.cursor.0-self.last_cursor.0) as f32/dt).clamp(-900.0*s,900.0*s),
                        ((input.cursor.1-self.last_cursor.1) as f32/dt).clamp(-900.0*s,900.0*s));
                }
            } else {
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
            self.velocity.1 += 1100.0*s*dt;
            x += self.velocity.0*dt;
            y += self.velocity.1*dt;
            if x < sx as f32+radius || x > (sx+sw) as f32-radius {
                x = x.clamp(sx as f32+radius,(sx+sw) as f32-radius);
                self.velocity.0 *= -0.6;
            }
            if y < sy as f32+radius { y = sy as f32+radius; self.velocity.1 = self.velocity.1.abs()*0.6; }
            if y >= (sy+sh) as f32-radius {
                y = (sy+sh) as f32-radius;
                self.velocity.1 = if self.velocity.1.abs() > 70.0*s { -self.velocity.1*0.45 } else { 0.0 };
                self.velocity.0 *= 0.88_f32.powf(dt*60.0);
            }
            let tx = (x-(PET_X+PET_W/2) as f32*s).round() as i32;
            let ty = (y-(PET_Y+PET_H-14) as f32*s).round() as i32;
            let target = (tx.clamp(sx,sx+sw-input.win_size.0),ty.clamp(sy,sy+sh-input.win_size.1));
            let dx = target.0-input.win_pos.0;
            let dy = target.1-input.win_pos.1;
            let distance = (dx as f32).hypot(dy as f32);
            if distance <= 12.0*s && self.velocity.1.abs() < 120.0*s {
                self.phase = "returning";
            } else {
                out.movement = Some(step_toward(input.win_pos,target,self.chase_speed()*s*dt));
                out.direction = Some(dx >= 0);
            }
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
                if input.extra_animations && self.playful && !self.teased && (self.streak+1)%3==0 {
                    self.phase="teasing";self.teased=true;self.tease_ms=0;
                    out.direction=Some(self.carry_right);
                } else if input.extra_animations {
                    self.phase = "releasing";
                    self.release_ms = 0;
                    self.release_from = self.ball.unwrap();
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
            let t = (self.release_ms as f32/400.0).min(1.0);
            let floor = self.home.1 as f32+(PET_Y+PET_H-14) as f32*s;
            x = self.release_from.0;
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
