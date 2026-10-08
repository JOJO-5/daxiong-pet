//! Fast changes of gaze direction, measured in logical pixels around the pet.
use crate::atlas::Row;
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Feedback { pub phase: &'static str, pub x: i32, pub y: i32, }

#[derive(Default)]
pub struct Dizziness {
    clock: u64,
    cooldown_until: u64,
    samples: VecDeque<(u64,f32,f32)>,
    elapsed: Option<u64>,
}

#[derive(Default)]
pub struct Step {
    pub started: bool,
    pub frame: Option<(Row,usize)>,
    pub feedback: Option<Feedback>,
}

impl Dizziness {
    pub fn active(&self)->bool { self.elapsed.is_some() }
    pub fn cancel(&mut self) { self.elapsed=None;self.samples.clear(); }

    pub fn tick(&mut self,dt:u64,point:(f32,f32),allowed:bool,builtin:bool)->Step {
        self.clock=self.clock.saturating_add(dt);
        if !allowed || !builtin || dt>250 {self.cancel();return Step::default();}
        let mut started=false;
        if let Some(elapsed)=self.elapsed.as_mut() {
            *elapsed+=dt;
            if *elapsed>=3500 {self.cancel();return Step::default();}
        } else {
            if self.clock<self.cooldown_until {return Step::default();}
            while self.samples.front().is_some_and(|s|self.clock-s.0>1250) {self.samples.pop_front();}
            let radius=point.0.hypot(point.1);
            if radius>360.0 {self.samples.clear();return Step::default();}
            // Crossing the gaze dead zone should not erase the preceding fast turns.
            if radius<48.0 {return Step::default();}
            if self.samples.back().is_none_or(|s|(point.0-s.1).hypot(point.1-s.2)>=4.0) {
                self.samples.push_back((self.clock,point.0,point.1));
            }
            while self.samples.len()>96 {self.samples.pop_front();}
            if self.samples.len()<8 || self.clock-self.samples.front().unwrap().0<500 {return Step::default();}
            let mut path=0.0;let mut turn=0.0;
            let mut previous:Option<(f32,f32)>=None;
            for &(_,x,y) in &self.samples {
                if let Some((px,py))=previous {
                    path+=(x-px).hypot(y-py);
                    let delta=y.atan2(x)-py.atan2(px);
                    turn+=delta.sin().atan2(delta.cos()).abs();
                }
                previous=Some((x,y));
            }
            if path<850.0 || turn<7.0 {return Step::default();}
            self.elapsed=Some(0);self.samples.clear();self.cooldown_until=self.clock+15_000;started=true;
        }
        let t=self.elapsed.unwrap();
        let (row,col,phase,x,y)=if t<150 {(Row::BellyRoll,0,"wobble",144,93)}
        else if t<450 {(Row::Affection,1,"wobble",147,105)}
        else if t<750 {(Row::Affection,3,"wobble",150,109)}
        else if t<1100 {(Row::BellyRoll,1,"fall",143,141)}
        else if t<2500 {(Row::BellyRoll,2,"down",105,181)}
        else if t<2750 {(Row::BellyRoll,5,"recover",105,181)}
        else if t<3150 {(Row::BellyRoll,6,"recover",143,141)}
        else {(Row::BellyRoll,7,"recover",144,93)};
        Step {started,frame:Some((row,col)),feedback:Some(Feedback {phase,x,y})}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shake(d:&mut Dizziness,builtin:bool)->bool {
        (0..18).any(|i|d.tick(80,(if i%2==0 {-145.0} else {145.0},-110.0),true,builtin).started)
    }
    #[test]
    fn fast_shakes_and_circles_trigger_but_slow_motion_and_one_pass_do_not() {
        assert!(shake(&mut Dizziness::default(),true));
        let mut circle=Dizziness::default();let mut seen=false;
        for i in 0..35 {let a=i as f32*0.4;seen|=circle.tick(35,(a.cos()*130.0,a.sin()*130.0),true,true).started;}
        assert!(seen);
        let mut slow=Dizziness::default();
        for i in 0..300 {let a=i as f32*0.09;assert!(!slow.tick(100,(a.cos()*130.0,a.sin()*130.0),true,true).started);}
        let mut pass=Dizziness::default();
        for i in 0..30 {assert!(!pass.tick(25,(-145.0+i as f32*10.0,-110.0),true,true).started);}
    }
    #[test]
    fn complete_pose_reuses_existing_pixels_recovers_and_has_a_cooldown() {
        let mut d=Dizziness::default();assert!(shake(&mut d,true));
        let mut down=false;let mut recover=false;
        for _ in 0..230 {
            let s=d.tick(16,(0.0,0.0),true,true);
            if let Some(f)=s.feedback {down|=f.phase=="down";recover|=f.phase=="recover";assert!((36..264).contains(&f.x)&&(18..222).contains(&f.y));}
        }
        assert!(down&&recover);assert!(!d.active());assert!(!shake(&mut d,true));
        d.tick(15_000,(0.0,0.0),true,true);assert!(shake(&mut d,true));
    }
    #[test]
    fn blocked_gaze_and_suspend_clear_samples_and_active_feedback() {
        let mut d=Dizziness::default();
        for i in 0..6 {d.tick(80,(if i%2==0 {-145.0} else {145.0},-110.0),true,true);}
        d.tick(16,(0.0,0.0),false,true);
        assert!(!d.tick(80,(-145.0,-110.0),true,true).started);
        assert!(shake(&mut d,true));assert!(d.tick(16,(0.0,0.0),false,true).feedback.is_none());
        let mut d=Dizziness::default();assert!(shake(&mut d,true));
        assert!(d.tick(60_000,(145.0,-110.0),true,true).frame.is_none());assert!(!d.active());
    }
    #[test]
    fn legacy_pet_never_uses_extended_frames() {
        let mut d=Dizziness::default();assert!(!shake(&mut d,false));
        assert!(d.tick(16,(145.0,-110.0),true,false).frame.is_none());
        assert!(shake(&mut d,true));
        assert!(d.tick(16,(145.0,-110.0),true,false).feedback.is_none());
    }
}
