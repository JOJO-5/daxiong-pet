//! Display calibration for built-in poses, anchored to the same ground line.
//! Horizontal poses need more room than the 192px atlas cell. Scale the clipped
//! cell (never the atlas itself), and use this mapping for input and props too.
use crate::atlas::Row;
use crate::engine::{PET_X, PET_Y};

pub const PIVOT: (f32, f32) = (150.0, 236.25); // cell center x, source ground y=203

// One calibration table is shared by native rendering, audit and preview tools.
static CALIBRATION: std::sync::LazyLock<Vec<(u8,usize,usize,f32)>> = std::sync::LazyLock::new(|| {
    include_str!("../../assets/animation-source/pose-scales.csv").lines().skip(1).map(|line| {
        let values:Vec<_>=line.split(',').collect();
        (values[0].parse().expect("pose row"),values[1].parse().expect("pose first column"),
         values[2].parse().expect("pose last column"),values[3].parse().expect("pose scale"))
    }).collect()
});

pub fn scale(row: Row, col: usize, builtin: bool) -> f32 {
    if !builtin { return 1.0; }
    CALIBRATION.iter().find(|&&(r,first,last,_)|r==row as u8 && (first..=last).contains(&col))
        .map_or(1.0,|entry|entry.3)
}

/// Unscaled window-local point → displayed window-local point.
pub fn display(point: (f32, f32), scale: f32) -> (f32, f32) {
    (PIVOT.0 + (point.0 - PIVOT.0) * scale,
     PIVOT.1 + (point.1 - PIVOT.1) * scale)
}

/// Displayed window-local point → original 144×156 touch coordinates.
pub fn touch(point: (f32, f32), scale: f32) -> (f32, f32) {
    ((point.0 - PIVOT.0) / scale + PIVOT.0 - PET_X as f32,
     (point.1 - PIVOT.1) / scale + PIVOT.1 - PET_Y as f32)
}

pub fn lying(row: Row, col: usize, builtin: bool) -> bool {
    builtin && (row == Row::BellyRub || (row == Row::BellyRoll && (2..=5).contains(&col)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enlarged_pose_keeps_ground_and_touch_locations_at_all_display_scales() {
        for s in [1.0, 1.1, 1.5] {
            assert_eq!(display(PIVOT, s), PIVOT);
            for p in [(8.0, 108.0), (64.0, 149.0), (130.0, 144.0)] {
                let visible=display((p.0+PET_X as f32,p.1+PET_Y as f32),s);
                for dpi in [1.0,1.25,2.0] {
                    let physical=(visible.0*dpi,visible.1*dpi);
                    let recovered=touch((physical.0/dpi,physical.1/dpi),s);
                    assert!((recovered.0-p.0).abs()<0.001 && (recovered.1-p.1).abs()<0.001);
                }
            }
        }
    }
    #[test]
    fn original_frames_legacy_pets_and_upright_transition_frames_keep_their_size() {
        for row in 0..27 {
            let row=Row::from_index(row).unwrap();
            for col in 0..8 {
                assert_eq!(scale(row,col,false),1.0);
                if !matches!(row,Row::BellyRub|Row::BellyRoll|Row::DiscRight|Row::DiscLeft) {
                    assert_eq!(scale(row,col,true),1.0);
                }
            }
        }
        for col in [0,1,6,7] {assert_eq!(scale(Row::BellyRoll,col,true),1.0);}
    }
}
