//! Clipping detection on the meters (SON-P17), with no UI: a track whose post-fader peak reaches
//! full scale is reported as clipping, one below it is not.

use sonara_engine::player::TrackMeter;

fn meter(peak: [f64; 2]) -> TrackMeter {
    TrackMeter {
        track_id: "trk_1".to_owned(),
        peak,
        rms: [0.0, 0.0],
    }
}

#[test]
fn a_track_meter_clips_at_full_scale_on_either_channel() {
    assert!(!meter([0.0, 0.0]).is_clipping());
    assert!(!meter([0.9999, 0.5]).is_clipping());
    assert!(meter([1.0, 0.0]).is_clipping());
    assert!(meter([0.2, 1.4]).is_clipping());
    assert!(meter([f64::NAN, 0.0]).is_clipping());
}
