//! Short, cancellable tricks use the existing atlas and physical desktop bounds.
use crate::{atlas::Row,engine::{Input,PET_X,PET_Y,PET_W,PET_H}};
use serde::Serialize;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="lowercase")]
pub enum Cue { Come, Spin, Down, Stay }
impl Cue {
    pub fn parse(s:&str)->Result<Self,String> {match s {"come"=>Ok(Self::Come),"spin"=>Ok(Self::Spin),"down"=>Ok(Self::Down),"stay"=>Ok(Self::Stay),_=>Err("未知指令".into())}}
    pub fn index(self)->usize {match self {Self::Come=>0,Self::Spin=>1,Self::Down=>2,Self::Stay=>3}}
    pub fn label(self)->&'static str {match self {Self::Come=>"过来",Self::Spin=>"转圈",Self::Down=>"趴下",Self::Stay=>"等一下"}}
}
#[derive(Clone,Debug,PartialEq,Serialize)]
pub struct ActivityView {pub id:u64,pub kind:Option<Cue>,pub phase:&'static str,pub rewardable:bool,pub game:&'static str,pub treat:Option<(i32,i32)>,pub difficulty:&'static str,pub finds:u32}
pub struct ActivityStep {pub movement:Option<(i32,i32)>,pub row:Option<Row>,pub col:Option<usize>,pub completed:bool}
pub struct Activities {view:ActivityView,elapsed:u64,attention:u64,home:(i32,i32),target:Option<(i32,i32)>,was_down:bool,held_offset:(i32,i32),waypoint:Option<(i32,i32)>,observe_until:u64,search_right:bool}
impl Default for Activities {fn default()->Self {Self {view:ActivityView{id:0,kind:None,phase:"off",rewardable:false,game:"none",treat:None,difficulty:"easy",finds:0},elapsed:0,attention:700,home:(0,0),target:None,was_down:false,held_offset:(0,0),waypoint:None,observe_until:0,search_right:true}}}
impl Activities {
    pub fn view(&self)->ActivityView {self.view.clone()}
    pub fn active(&self)->bool {matches!(self.view.phase,"attention" | "performing" | "placing" | "held" | "ready" | "searching" | "observing")}
    pub fn cancel(&mut self) {self.view.kind=None;self.view.phase="off";self.view.rewardable=false;self.target=None;self.view.treat=None;self.view.game="none";}
    pub fn blocked(&mut self) {self.cancel();self.view.phase="blocked";}
    pub fn start(&mut self,cue:Cue,input:&Input,learned:bool) {
        self.view=ActivityView{id:self.view.id.saturating_add(1),kind:Some(cue),phase:"attention",rewardable:false,game:"trick",treat:None,difficulty:"easy",finds:self.view.finds};
        self.elapsed=0;self.attention=if learned {300} else {700};self.home=input.win_pos;self.target=None;
    }
    fn bound(input:&Input,pos:(i32,i32))->(i32,i32) {
        let (sx,sy,sw,sh)=input.screen;
        (pos.0.clamp(sx,sx+(sw-input.win_size.0).max(0)),pos.1.clamp(sy,sy+(sh-input.win_size.1).max(0)))
    }
    pub fn pointer_hot(&self,cursor:(i32,i32),scale:f64)->bool {
        self.view.treat.is_some_and(|p| ((cursor.0-p.0) as f64).hypot((cursor.1-p.1) as f64)<=18.0*scale)
    }
    fn bound_treat(input:&Input,p:(i32,i32))->(i32,i32) {
        let s=input.scale_factor as f32;let (sx,sy,sw,sh)=input.screen;
        let x=((PET_X+PET_W/2) as f32*s).round() as i32;
        let y=((PET_Y+PET_H-14) as f32*s).round() as i32;
        (p.0.clamp(sx+x,sx+(sw-input.win_size.0).max(0)+x),p.1.clamp(sy+y,sy+(sh-input.win_size.1).max(0)+y))
    }
    pub fn place_snack(&mut self,input:&Input,far:bool) {
        let s=input.scale_factor as f32;let (sx,_,sw,_)=input.screen;
        let center=input.win_pos.0+((PET_X+PET_W/2) as f32*s) as i32;
        let dir=if center<sx+sw/2 {1.0} else {-1.0};
        let offset=if far {230.0} else {75.0};
        let treat=Self::bound_treat(input,(center+(dir*offset*s) as i32,input.win_pos.1+((PET_Y+PET_H-14) as f32*s) as i32));
        self.view=ActivityView{id:self.view.id.saturating_add(1),kind:None,phase:"placing",rewardable:false,game:"snack",treat:Some(treat),difficulty:if far {"far"} else {"easy"},finds:self.view.finds};
        self.elapsed=0;self.target=None;self.waypoint=None;self.was_down=input.button_down;self.home=input.win_pos;
    }
    pub fn find_snack(&mut self,input:&Input) {
        if self.view.game!="snack" || !matches!(self.view.phase,"placing" | "ready") {return;}
        let p=self.view.treat.unwrap();let s=input.scale_factor as f32;
        self.search_right=p.0>=input.win_pos.0+((PET_X+PET_W/2) as f32*s) as i32;
        let anchor=if self.search_right {PET_X+PET_W-18} else {PET_X+18};
        let target=Self::bound(input,(p.0-(anchor as f32*s) as i32,p.1-((PET_Y+PET_H-14) as f32*s) as i32));
        self.target=Some(target);self.waypoint=if self.view.difficulty=="far" {Some(Self::bound(input,((target.0+input.win_pos.0)/2,((target.1+input.win_pos.1)/2)-(50.0*s) as i32)))} else {None};
        self.attention=if self.view.difficulty=="far" {1100} else {600};self.elapsed=0;self.view.phase="attention";self.was_down=input.button_down;
    }
    fn snack_tick(&mut self,input:&Input)->ActivityStep {
        let mut out=ActivityStep{movement:None,row:Some(Row::Review),col:Some(0),completed:false};
        let s=input.scale_factor as f32;
        if input.button_down && !self.was_down && self.pointer_hot(input.cursor,input.scale_factor) {
            let p=self.view.treat.unwrap();self.held_offset=(p.0-input.cursor.0,p.1-input.cursor.1);self.view.phase="held";self.target=None;self.waypoint=None;
        }
        self.was_down=input.button_down;
        if self.view.phase=="held" {
            self.view.treat=Some(Self::bound_treat(input,(input.cursor.0+self.held_offset.0,input.cursor.1+self.held_offset.1)));
            if !input.button_down {self.view.phase="ready";self.elapsed=0;}
            return out;
        }
        if matches!(self.view.phase,"placing" | "ready") {return out;}
        if self.view.phase=="attention" {
            out.row=Some(if !input.look_enabled {Row::Review} else if self.search_right {Row::LookA} else {Row::LookB});out.col=Some(if input.look_enabled {4} else {0});
            if self.elapsed>=self.attention {self.view.phase="searching";}return out;
        }
        if self.view.phase=="observing" {
            out.col=Some(3);if self.elapsed>=self.observe_until {self.view.phase="searching";}return out;
        }
        if self.elapsed>15_000 {self.cancel();self.view.phase="cancelled";return out;}
        let target=Self::bound(input,self.waypoint.or(self.target).unwrap());
        let dx=(target.0-input.win_pos.0) as f32;let dy=(target.1-input.win_pos.1) as f32;let d=dx.hypot(dy);
        let step=200.0*s*input.dt_ms.min(50) as f32/1000.0;
        let pos=if d<=step || d<1.0 {target} else {(input.win_pos.0+(dx/d*step).round() as i32,input.win_pos.1+(dy/d*step).round() as i32)};
        out.movement=Some(Self::bound(input,pos));out.row=Some(if dx>=0.0 {Row::RunRight} else {Row::RunLeft});out.col=None;
        if d<8.0*s {
            if self.waypoint.take().is_some() {self.view.phase="observing";self.observe_until=self.elapsed+600;}
            else {self.view.phase="found";self.view.treat=None;self.view.finds=self.view.finds.saturating_add(1);out.completed=true;out.row=Some(if input.extra_animations {Row::EatTreat} else {Row::Waving});}
        }
        out
    }
    pub fn tick(&mut self,input:&Input)->ActivityStep {
        let mut out=ActivityStep{movement:None,row:None,col:None,completed:false};
        if !self.active() {return out;}
        if !input.interactive || input.screen.2<input.win_size.0 || input.screen.3<input.win_size.1 {self.cancel();return out;}
        self.elapsed=self.elapsed.saturating_add(input.dt_ms);
        let s=input.scale_factor as f32;
        if self.view.game=="snack" {return self.snack_tick(input);}
        let cue=self.view.kind.unwrap();
        if self.elapsed<self.attention {
            if !input.look_enabled {out.row=Some(Row::Review);out.col=Some(0);return out;}
            let dx=input.cursor.0-(input.win_pos.0+((PET_X+PET_W/2) as f32*s) as i32);
            let dy=input.cursor.1-(input.win_pos.1+((PET_Y+PET_H/2) as f32*s) as i32);
            let angle=(dx as f32).atan2(-(dy as f32)).to_degrees().rem_euclid(360.0);
            let index=((angle/22.5).round() as usize)%16;
            out.row=Some(if index<8 {Row::LookA} else {Row::LookB});out.col=Some(index%8);return out;
        }
        self.view.phase="performing";
        let t=self.elapsed-self.attention;
        let done=match cue {
            Cue::Come=>{
                let target=*self.target.get_or_insert_with(||Self::bound(input,(input.cursor.0-((PET_X+PET_W/2) as f32*s) as i32,input.cursor.1-((PET_Y+PET_H/2) as f32*s) as i32)));
                let target=Self::bound(input,target);
                let dx=(target.0-input.win_pos.0) as f32;let dy=(target.1-input.win_pos.1) as f32;let d=dx.hypot(dy);
                let step=240.0*s*input.dt_ms.min(50) as f32/1000.0;
                let pos=if d<=step || d<1.0 {target} else {(input.win_pos.0+(dx/d*step).round() as i32,input.win_pos.1+(dy/d*step).round() as i32)};
                out.movement=Some(Self::bound(input,pos));out.row=Some(if dx>=0.0 {Row::RunRight} else {Row::RunLeft});
                if t>12_000 && d>8.0*s {self.cancel();self.view.phase="cancelled";return out;}
                d<=8.0*s
            }
            Cue::Spin=>{
                let angle=(t.min(1600) as f32/1600.0)*std::f32::consts::TAU;
                let pos=(self.home.0+(angle.sin()*28.0*s).round() as i32,self.home.1+((angle.cos()-1.0)*28.0*s).round() as i32);
                out.movement=Some(Self::bound(input,pos));out.row=Some(if angle.cos()>=0.0 {Row::RunRight} else {Row::RunLeft});t>=1600
            }
            Cue::Down=>{out.row=Some(if input.extra_animations {Row::Sleep} else {input.sleep_frame.0});out.col=Some(if input.extra_animations {0} else {input.sleep_frame.1});t>=1800}
            Cue::Stay=>{out.row=Some(Row::Waiting);out.col=Some(0);t>=2200}
        };
        if done {out.completed=true;self.view.phase="completed";self.view.rewardable=true;out.row=Some(if input.extra_animations {Row::HappyPat} else {Row::Waving});out.col=None;}
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input(scale:f64)->Input {Input{dt_ms:16,interactive:true,cursor:(-700,550),win_pos:(-1300,300),win_size:((300.0*scale) as i32,(240.0*scale) as i32),scale_factor:scale,screen:(-1920,0,1920,1080),button_down:false,look_enabled:true,extra_animations:true,encounters_enabled:true,gravity:false,local_hour:12,sleep_frame:(Row::Failed,2)}}
    #[test]
    fn long_pause_cancels_stalled_search_and_come_without_reward() {
        let mut i=input(1.0);i.dt_ms=30_000;let mut a=Activities::default();
        a.start(Cue::Come,&i,false);a.tick(&i);assert_eq!(a.view().phase,"cancelled");assert!(!a.view().rewardable);
        a.place_snack(&i,true);a.find_snack(&i);a.tick(&i);a.tick(&i);
        assert_eq!(a.view().phase,"cancelled");assert!(a.view().treat.is_none());assert_eq!(a.view().finds,0);
    }
    #[test]
    fn nine_row_pets_never_require_look_or_extension_rows() {
        let mut i=input(1.0);i.extra_animations=false;i.look_enabled=false;
        let mut a=Activities::default();
        for cue in [Cue::Come,Cue::Spin,Cue::Down,Cue::Stay] {
            a.start(cue,&i,false);
            for _ in 0..1000 {let step=a.tick(&i);assert!(step.row.is_none_or(|r|(r as u8)<9));if let Some(pos)=step.movement {i.win_pos=pos;}if a.view().phase=="completed" {break;}}
            assert_eq!(a.view().phase,"completed");
        }
        a.place_snack(&i,true);a.find_snack(&i);
        for _ in 0..1200 {let step=a.tick(&i);assert!(step.row.is_none_or(|r|(r as u8)<9));if let Some(pos)=step.movement {i.win_pos=pos;}if a.view().phase=="found" {break;}}
        assert_eq!(a.view().phase,"found");
    }
    #[test]
    fn snacks_drag_within_reachable_bounds_find_once_and_cancel() {
        for scale in [1.0,1.25,2.0] {for far in [false,true] {
            let mut i=input(scale);let mut a=Activities::default();a.place_snack(&i,far);
            i.cursor=a.view().treat.unwrap();i.button_down=true;a.tick(&i);assert_eq!(a.view().phase,"held");
            i.cursor=(-9999,9999);a.tick(&i);let p=a.view().treat.unwrap();assert!(p.0>=-1920 && p.1<=1080);
            i.button_down=false;a.tick(&i);assert_eq!(a.view().phase,"ready");a.find_snack(&i);
            let mut completed=0;let mut observed=false;
            for _ in 0..1500 {let step=a.tick(&i);completed+=usize::from(step.completed);observed|=a.view().phase=="observing";if let Some(pos)=step.movement {i.win_pos=pos;}if a.view().phase=="found" {break;}}
            assert_eq!(a.view().phase,"found");assert!(a.view().treat.is_none());assert_eq!(completed,1);assert_eq!(a.view().finds,1);assert_eq!(observed,far);
            assert!(!a.tick(&i).completed);assert!(!a.view().rewardable);
            a.place_snack(&i,far);i.interactive=false;a.tick(&i);assert_eq!(a.view().phase,"off");assert!(a.view().treat.is_none());
        }}
    }
    #[test]
    fn all_tricks_complete_with_bounds_and_legacy_frames() {
        for scale in [1.0,1.25,2.0] {for builtin in [false,true] {for cue in [Cue::Come,Cue::Spin,Cue::Down,Cue::Stay] {
            let mut i=input(scale);i.extra_animations=builtin;let mut a=Activities::default();a.start(cue,&i,false);let home=i.win_pos;
            for _ in 0..1000 {
                let step=a.tick(&i);if let Some(row)=step.row {assert!(builtin || (row as u8)<11);}
                if let Some(p)=step.movement {assert!(p.0>=-1920 && p.0<=-i.win_size.0 && p.1>=0 && p.1<=1080-i.win_size.1);i.win_pos=p;}
                if a.view().phase=="completed" {break;}
            }
            assert_eq!(a.view().phase,"completed");assert!(a.view().rewardable);
            if matches!(cue,Cue::Down | Cue::Stay | Cue::Spin) {assert_eq!(i.win_pos,home);}
            a.cancel();assert!(!a.active());assert!(!a.view().rewardable);
        }}}
    }
    #[test]
    fn learned_attention_is_shorter_and_hidden_commands_cancel() {
        let mut i=input(1.0);i.dt_ms=100;let mut a=Activities::default();a.start(Cue::Stay,&i,true);
        for _ in 0..3 {a.tick(&i);}assert_eq!(a.view().phase,"performing");
        a.start(Cue::Stay,&i,false);for _ in 0..3 {a.tick(&i);}assert_eq!(a.view().phase,"attention");
        i.interactive=false;a.tick(&i);assert_eq!(a.view().phase,"off");assert!(!a.view().rewardable);
    }
}
