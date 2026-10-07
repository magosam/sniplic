use super::*;

#[test]
fn new_project_has_empty_subtitles() {
    let p = Project::new("Test Project".to_string());
    assert!(p.subtitles.is_some());
    let subs = p.subtitles.unwrap();
    assert_eq!(subs.items.len(), 0);
}

#[test]
fn track_height_serde_and_resilience() {
    // 1. Deserialization of legacy JSON without height field
    let legacy_track_json = r#"{
        "id": "v1", "name": "V1", "track_type": "Video",
        "locked": false, "muted": false, "hidden": false, "solo": false, "clips": []
    }"#;
    let track_legacy: Track = serde_json::from_str(legacy_track_json).unwrap();
    assert_eq!(track_legacy.height, None);

    // 2. Deserialization with integer
    let int_track_json = r#"{
        "id": "v1", "name": "V1", "track_type": "Video",
        "locked": false, "muted": false, "hidden": false, "solo": false, "clips": [],
        "height": 92
    }"#;
    let track_int: Track = serde_json::from_str(int_track_json).unwrap();
    assert_eq!(track_int.height, Some(92));

    // 3. Resilient deserialization with float (from TS frontend)
    let float_track_json = r#"{
        "id": "v1", "name": "V1", "track_type": "Video",
        "locked": false, "muted": false, "hidden": false, "solo": false, "clips": [],
        "height": 91.6
    }"#;
    let track_float: Track = serde_json::from_str(float_track_json).unwrap();
    assert_eq!(track_float.height, Some(92));

    // 4. Roundtrip serialization
    let serialized = serde_json::to_string(&track_int).unwrap();
    assert!(serialized.contains(r#""height":92"#));
    let recovered: Track = serde_json::from_str(&serialized).unwrap();
    assert_eq!(recovered.height, Some(92));
}

#[test]
fn legacy_json_deserializes_subtitles_as_none() {
    let json = r#"{
        "metadata": { "id": "p1", "name": "Legacy", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": []
    }"#;
    let p: Result<Project, _> = serde_json::from_str(json);
    assert!(p.is_ok());
    let project = p.unwrap();
    assert_eq!(project.subtitles, None);
}

#[test]
fn project_revision_default_and_bump() {
    let json = r#"{
        "metadata": { "id": "p1", "name": "Legacy", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": []
    }"#;
    let mut project: Project = serde_json::from_str(json).unwrap();
    assert_eq!(project.metadata.revision, 1);

    let rev1 = project.bump_revision();
    assert_eq!(rev1, 2);
    assert_eq!(project.metadata.revision, 2);

    let rev2 = project.bump_revision();
    assert_eq!(rev2, 3);
    assert_eq!(project.metadata.revision, 3);

    let serialized = serde_json::to_string(&project).unwrap();
    let recovered: Project = serde_json::from_str(&serialized).unwrap();
    assert_eq!(recovered.metadata.revision, 3);
}

#[test]
fn project_with_subtitles_serializes_and_deserializes_correctly() {
    let mut p = Project::new("With Subs".to_string());
    p.subtitles = Some(ProjectSubtitles {
        items: vec![SubtitleBlock {
            id: "sub_1".to_string(),
            start_frame: 0,
            end_frame: 60,
            text: "Olá mundo".to_string(),
            confidence: Some(0.95),
            track_index: None,
            ..Default::default()
        }],
        position: SubtitlePosition { x: 50.0, y: 80.0 },
        width: 70.0,
        font_size: 20.0,
        max_chars_per_block: 40,
        style: "standard".to_string(),
        language: "pt-BR".to_string(),
        ..Default::default()
    });

    let json = serde_json::to_string(&p).expect("failed to serialize");
    assert!(json.contains(r#""subtitles""#));
    assert!(json.contains(r#""startFrame":0"#));
    assert!(json.contains(r#""endFrame":60"#));
    assert!(json.contains(r#""text":"Olá mundo""#));

    let recovered: Project = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(recovered.subtitles, p.subtitles);
}

#[test]
fn clip_transition_persistence_serde() {
    let json = r#"{
        "metadata": { "id": "p1", "name": "Transition Test", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": [
            {
                "id": "t1",
                "name": "Video 1",
                "track_type": "Video",
                "locked": false,
                "muted": false,
                "hidden": false,
                "solo": false,
                "clips": [
                    {
                        "id": "c1",
                        "media_id": "m1",
                        "name": "Clip 1",
                        "start_frame": 0,
                        "duration_frames": 100,
                        "in_point_frames": 0,
                        "out_point_frames": 100,
                        "transform": {
                            "position_x": 0.0,
                            "position_y": 0.0,
                            "scale_x": 1.0,
                            "scale_y": 1.0,
                            "rotation_degrees": 0.0,
                            "opacity": 1.0
                        },
                        "audio": {
                            "volume": 1.0,
                            "muted": false
                        },
                        "transitionPluginData": {
                            "transitionId": "basic.slide.up",
                            "durationFrames": 24,
                            "direction": "up"
                        }
                    }
                ]
            }
        ]
    }"#;
    let p: Result<Project, _> = serde_json::from_str(json);
    assert!(p.is_ok(), "Failed to deserialize: {:?}", p.err());
    let proj = p.unwrap();
    let clip = &proj.tracks[0].clips[0];
    assert!(clip.transition_plugin_data.is_some());
    let trans = clip.transition_plugin_data.as_ref().unwrap();
    assert_eq!(trans.get("transitionId").and_then(|v| v.as_str()), Some("basic.slide.up"));

    // Roundtrip serialization
    let serialized = serde_json::to_string_pretty(&proj).expect("Failed to serialize");
    let reloaded: Project = serde_json::from_str(&serialized).expect("Failed to reload");
    let reloaded_clip = &reloaded.tracks[0].clips[0];
    assert!(reloaded_clip.transition_plugin_data.is_some());

    // Also test with snake_case aliases from frontend
    let json_snake = r#"{
        "metadata": { "id": "p2", "name": "Snake Case Test", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": [
            {
                "id": "t1",
                "name": "Video 1",
                "track_type": "Video",
                "locked": false,
                "muted": false,
                "hidden": false,
                "solo": false,
                "clips": [
                    {
                        "id": "c2",
                        "media_id": "m1",
                        "name": "Clip 2",
                        "start_frame": 0,
                        "duration_frames": 60,
                        "in_point_frames": 0,
                        "out_point_frames": 60,
                        "transform_plugin_data": {
                            "core.transitions.basic": {
                                "transitionId": "gl.shape.circle",
                                "durationFrames": 30
                            }
                        },
                        "transition_plugin_data": {
                            "transitionId": "gl.shape.circle",
                            "durationFrames": 30
                        }
                    }
                ]
            }
        ]
    }"#;
    let p_snake: Result<Project, _> = serde_json::from_str(json_snake);
    assert!(p_snake.is_ok(), "Failed to deserialize snake_case: {:?}", p_snake.err());
    let proj_snake = p_snake.unwrap();
    let clip_snake = &proj_snake.tracks[0].clips[0];
    assert!(clip_snake.transition_plugin_data.is_some());
    assert!(clip_snake.transform_plugin_data.is_some());
}

#[test]
fn test_clip_with_both_camel_and_snake_keys() {
    let json = r#"{
        "metadata": { "id": "p_both", "name": "Both Keys", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": [
            {
                "id": "t1",
                "name": "Video 1",
                "track_type": "Video",
                "locked": false,
                "muted": false,
                "hidden": false,
                "solo": false,
                "clips": [
                    {
                        "id": "c_both",
                        "media_id": "m1",
                        "name": "Clip Both",
                        "start_frame": 0,
                        "duration_frames": 60,
                        "in_point_frames": 0,
                        "out_point_frames": 60,
                        "transformPluginData": { "core.transitions.basic": { "transitionId": "gl.shape.circle" } },
                        "transform_plugin_data": { "core.transitions.basic": { "transitionId": "gl.shape.circle" } },
                        "transitionPluginData": { "transitionId": "gl.shape.circle", "durationFrames": 30 },
                        "transition_plugin_data": { "transitionId": "gl.shape.circle", "durationFrames": 30 }
                    }
                ]
            }
        ]
    }"#;
    let p: Result<Project, _> = serde_json::from_str(json);
    println!("Result: {:?}", p.as_ref().err());
    assert!(p.is_ok(), "Failed: {:?}", p.err());
}

#[test]
fn subtitles_deserializes_floats_and_integers_safely() {
    let json = r#"{
        "metadata": { "id": "p_subs_flex", "name": "Float Subs", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": [],
        "subtitles": {
            "items": [
                {
                    "id": "sub_float_1",
                    "startFrame": 15.4,
                    "endFrame": 62.8,
                    "text": "Legenda com float",
                    "confidence": 0.99
                },
                {
                    "id": "sub_snake_2",
                    "start_frame": 70,
                    "end_frame": 120.0,
                    "text": "Legenda snake com float",
                    "confidence": 0.95
                }
            ],
            "position": { "x": 50.0, "y": 85.0 },
            "width": 80.0,
            "fontSize": 24.0,
            "maxCharsPerBlock": 45.0,
            "style": "standard",
            "language": "pt-BR"
        }
    }"#;

    let res: Result<Project, _> = serde_json::from_str(json);
    assert!(res.is_ok(), "Failed to deserialize subtitles with float numbers: {:?}", res.err());
    let p = res.unwrap();
    let subs = p.subtitles.expect("subtitles should exist");
    assert_eq!(subs.items.len(), 2);
    assert_eq!(subs.items[0].start_frame, 15);
    assert_eq!(subs.items[0].end_frame, 63);
    assert_eq!(subs.items[1].start_frame, 70);
    assert_eq!(subs.items[1].end_frame, 120);
    assert_eq!(subs.max_chars_per_block, 45);
}

#[test]
fn test_subtitles_with_both_camel_and_snake_keys() {
    let json = r#"{
        "metadata": { "id": "p_subs_both", "name": "Both Subs", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "tracks": [],
        "subtitles": {
            "items": [
                {
                    "id": "sub_both_1",
                    "startFrame": 10,
                    "start_frame": 10,
                    "endFrame": 50,
                    "end_frame": 50,
                    "text": "Texto duplicado",
                    "confidence": 0.99
                }
            ]
        }
    }"#;

    let res: Result<Project, _> = serde_json::from_str(json);
    assert!(res.is_ok(), "Failed with duplicate keys in subtitles: {:?}", res.err());
    let p = res.unwrap();
    let subs = p.subtitles.expect("subtitles should exist");
    assert_eq!(subs.items.len(), 1);
    assert_eq!(subs.items[0].start_frame, 10);
    assert_eq!(subs.items[0].end_frame, 50);
}

#[test]
fn test_load_real_project_file() {
    let path = std::path::Path::new(".editor_cache/projects/prj_6aa2ebc3.json");
    println!("Path exists: {}", path.exists());
    if path.exists() {
        let content = std::fs::read_to_string(path).unwrap();
        let p: Result<Project, _> = serde_json::from_str(&content);
        println!("Deserialization of prj_6aa2ebc3: {:?}", p.as_ref().err());
        assert!(p.is_ok());

        // Now test adding a subtitle block exactly like JS does
        let mut v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let new_sub = serde_json::json!({
            "id": "sub_123456789_abcd",
            "startFrame": 30,
            "endFrame": 90,
            "text": "Nova legenda adicionada",
            "confidence": 0.98
        });
        v["subtitles"]["items"] = serde_json::json!([new_sub]);

        let p_with_sub: Result<Project, _> = serde_json::from_value(v.clone());
        println!("Deserialization with added subtitle: {:?}", p_with_sub.as_ref().err());
        assert!(p_with_sub.is_ok());
    }
}

#[test]
fn test_duplicate_fields_in_project() {
    let json = r#"{
        "metadata": { "id": "p_dup", "name": "Dup", "version": "1.0.0", "created_at": 100, "updated_at": 100 },
        "config": { "width": 1920, "height": 1080, "fps": 30.0, "sample_rate": 48000, "audio_channels": 2 },
        "media_pool": {},
        "mediaPool": {},
        "media_folders": [],
        "mediaFolders": [],
        "tracks": []
    }"#;
    let res: Result<Project, _> = serde_json::from_str(json);
    println!("Duplicate fields result: {:?}", res.as_ref().err());
    assert!(res.is_ok(), "Duplicate fields failed: {:?}", res.err());
}

#[test]
fn test_subtitle_save_and_reload_persistence() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_prj_subtitles_persistence.json");

    // 1. Create project with subtitles
    let mut proj = Project::new("Subtitle Persistence Test".to_string());
    let sub1 = SubtitleBlock {
        id: "sub_test_1".to_string(),
        start_frame: 0,
        end_frame: 60,
        text: "Primeira legenda do usuário".to_string(),
        confidence: Some(0.99),
        track_index: None,
        ..Default::default()
    };
    let sub2 = SubtitleBlock {
        id: "sub_test_2".to_string(),
        start_frame: 65,
        end_frame: 150,
        text: "Segunda legenda do usuário".to_string(),
        confidence: Some(0.95),
        track_index: Some(1),
        ..Default::default()
    };
    proj.subtitles = Some(ProjectSubtitles {
        items: vec![sub1.clone(), sub2.clone()],
        position: SubtitlePosition { x: 50.0, y: 80.0 },
        font_size: 32.0,
        color: "#ffffff".to_string(),
        style: "karaoke".to_string(),
        ..Default::default()
    });

    // 2. Save to file
    proj.save_to_file(&temp_file).expect("save_to_file should succeed");

    // 3. Reload from file
    let loaded = Project::load_from_file(&temp_file).expect("load_from_file should succeed");
    let loaded_subs = loaded.subtitles.as_ref().expect("subtitles must be present");
    assert_eq!(loaded_subs.items.len(), 2);
    assert_eq!(loaded_subs.items[0].text, "Primeira legenda do usuário");
    assert_eq!(loaded_subs.items[1].text, "Segunda legenda do usuário");
    assert_eq!(loaded_subs.position.y, 80.0);
    assert_eq!(loaded_subs.font_size, 32.0);
    assert_eq!(loaded_subs.style, "karaoke");

    // 4. Edit (delete the first, modify the second)
    let mut modified = loaded;
    let mut mod_subs = modified.subtitles.unwrap();
    mod_subs.items.remove(0); // delete sub1
    mod_subs.items[0].text = "Segunda legenda EDITADA pelo usuário".to_string();
    modified.subtitles = Some(mod_subs);

    // Save again
    modified.save_to_file(&temp_file).expect("save modified should succeed");

    // Reload and validate that deletion and edit persisted 100%
    let reloaded = Project::load_from_file(&temp_file).expect("reload modified should succeed");
    let reloaded_subs = reloaded.subtitles.expect("subtitles must exist");
    assert_eq!(reloaded_subs.items.len(), 1);
    assert_eq!(reloaded_subs.items[0].id, "sub_test_2");
    assert_eq!(reloaded_subs.items[0].text, "Segunda legenda EDITADA pelo usuário");

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}


