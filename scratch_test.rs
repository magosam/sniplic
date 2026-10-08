use sniplic_core::core::project::{Clip, MediaType, Project, Track, TrackType, ProjectMedia};
use sniplic_core::core::timeline::MovementOperations;
use sniplic_core::core::timeline::MoveClipItem;

fn main() {
    let mut project = Project::default();
    
    // Add V1 and V2
    let mut v1 = Track {
        id: "V1".to_string(),
        track_type: TrackType::Video,
        name: "V1".to_string(),
        clips: vec![],
        locked: false,
        visible: true,
        height: 0,
        settings: Default::default(),
    };
    
    let mut v2 = Track {
        id: "V2".to_string(),
        track_type: TrackType::Video,
        name: "V2".to_string(),
        clips: vec![],
        locked: false,
        visible: true,
        height: 0,
        settings: Default::default(),
    };

    project.media_pool.insert("m1".to_string(), ProjectMedia {
        id: "m1".to_string(),
        media_type: MediaType::Video,
        name: "m1".to_string(),
        file_path: "".to_string(),
        duration_frames: 10,
        metadata: Default::default(),
    });

    let c = Clip { id: "C".to_string(), media_id: "m1".to_string(), start_frame: 10, duration_frames: 10, out_point_frames: 10, ..Default::default() };
    v1.clips.push(c);

    let d = Clip { id: "D".to_string(), media_id: "m1".to_string(), start_frame: 0, duration_frames: 5, out_point_frames: 5, ..Default::default() };
    v2.clips.push(d);

    project.tracks.push(v1);
    project.tracks.push(v2);

    let moves = vec![MoveClipItem {
        clip_id: "D".to_string(),
        target_track_id: "V1".to_string(),
        new_start_frame: 15,
        near_track_id: None,
    }];

    MovementOperations::move_clips_batch(&mut project, &moves, true, true).unwrap();

    println!("AFTER V1:");
    for clip in project.tracks.iter().find(|t| t.id == "V1").unwrap().clips.iter() {
        println!("{} ({}..{})", clip.id, clip.start_frame, clip.start_frame + clip.duration_frames);
    }
}
