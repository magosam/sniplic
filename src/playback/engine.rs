use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Clone, serde::Serialize)]
pub struct PlaybackTickEvent {
    pub frame: u64,
    pub is_playing: bool,
}

pub struct PlaybackEngine {
    is_playing: Arc<AtomicBool>,
    current_frame: Arc<AtomicU64>,
    seek_requested: Arc<AtomicBool>,
    fps: Arc<Mutex<f64>>,
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    loop_range: Arc<Mutex<Option<(u64, u64)>>>,
    max_frame: Arc<AtomicU64>,
    event_sender: broadcast::Sender<PlaybackTickEvent>,
}

impl PlaybackEngine {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(16);
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_frame: Arc::new(AtomicU64::new(0)),
            seek_requested: Arc::new(AtomicBool::new(false)),
            fps: Arc::new(Mutex::new(30.0)),
            task_handle: Arc::new(Mutex::new(None)),
            loop_range: Arc::new(Mutex::new(None)),
            max_frame: Arc::new(AtomicU64::new(0)),
            event_sender: tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PlaybackTickEvent> {
        self.event_sender.subscribe()
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn get_current_frame(&self) -> u64 {
        self.current_frame.load(Ordering::SeqCst)
    }

    pub fn set_current_frame(&self, frame: u64) {
        self.current_frame.store(frame, Ordering::SeqCst);
    }

    pub async fn set_fps(&self, new_fps: f64) {
        let mut guard = self.fps.lock().await;
        *guard = if new_fps > 0.0 { new_fps } else { 30.0 };
    }

    pub fn set_max_frame(&self, max: u64) {
        self.max_frame.store(max, Ordering::SeqCst);
    }

    pub fn get_max_frame(&self) -> u64 {
        self.max_frame.load(Ordering::SeqCst)
    }

    pub async fn get_loop_range(&self) -> Option<(u64, u64)> {
        *self.loop_range.lock().await
    }

    pub async fn set_loop_range(&self, in_frame: Option<u64>, out_frame: Option<u64>) {
        let mut seek_to: Option<u64> = None;
        {
            let mut guard = self.loop_range.lock().await;
            match (in_frame, out_frame) {
                (Some(start), Some(end)) if end > start => {
                    *guard = Some((start, end));
                    let curr = self.current_frame.load(Ordering::SeqCst);
                    if curr < start || curr > end {
                        seek_to = Some(start);
                    }
                }
                _ => {
                    *guard = None;
                }
            }
        }
        if let Some(target) = seek_to {
            self.seek(target).await;
        }
    }

    pub async fn play(&self) {
        if self.is_playing.load(Ordering::SeqCst) {
            return;
        }

        self.is_playing.store(true, Ordering::SeqCst);
        self.seek_requested.store(false, Ordering::SeqCst);
        let is_playing_clone = self.is_playing.clone();
        let current_frame_clone = self.current_frame.clone();
        let seek_requested_clone = self.seek_requested.clone();
        let loop_range_clone = self.loop_range.clone();
        let max_frame_clone = self.max_frame.clone();
        let fps_val = *self.fps.lock().await;
        let event_sender_clone = self.event_sender.clone();

        let mut handle_guard = self.task_handle.lock().await;
        if let Some(old_handle) = handle_guard.take() {
            old_handle.abort();
        }

        let join_handle = tokio::spawn(async move {
            let mut start_instant = std::time::Instant::now();
            let mut start_frame = current_frame_clone.load(Ordering::SeqCst);
            let mut frame_count: u64 = 0;

            while is_playing_clone.load(Ordering::SeqCst) {
                // If a seek was requested, immediately re-anchor clock to target frame
                if seek_requested_clone.swap(false, Ordering::SeqCst) {
                    start_frame = current_frame_clone.load(Ordering::SeqCst);
                    start_instant = std::time::Instant::now();
                    frame_count = 0;
                }

                frame_count += 1;
                let target_time = start_instant + Duration::from_secs_f64(frame_count as f64 / fps_val);
                let now = std::time::Instant::now();
                if target_time > now {
                    tokio::time::sleep(target_time - now).await;
                }

                if !is_playing_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Check again if a seek occurred while sleeping
                if seek_requested_clone.swap(false, Ordering::SeqCst) {
                    start_frame = current_frame_clone.load(Ordering::SeqCst);
                    start_instant = std::time::Instant::now();
                    frame_count = 0;
                    continue;
                }

                // Strict synchronization with wall clock to eliminate drift and jitter
                let elapsed = start_instant.elapsed().as_secs_f64();
                let mut actual_frame = start_frame + (elapsed * fps_val).round() as u64;

                let loop_opt = *loop_range_clone.lock().await;
                if let Some((loop_in, loop_out)) = loop_opt {
                    if actual_frame >= loop_out || actual_frame < loop_in {
                        actual_frame = loop_in;
                        current_frame_clone.store(loop_in, Ordering::SeqCst);
                        start_frame = loop_in;
                        start_instant = std::time::Instant::now();
                        frame_count = 0;
                    } else {
                        current_frame_clone.store(actual_frame, Ordering::SeqCst);
                    }
                } else {
                    let max_f = max_frame_clone.load(Ordering::SeqCst);
                    if max_f > 0 && actual_frame >= max_f {
                        current_frame_clone.store(max_f, Ordering::SeqCst);
                        is_playing_clone.store(false, Ordering::SeqCst);
                        let _ = event_sender_clone.send(
                            PlaybackTickEvent {
                                frame: max_f,
                                is_playing: false,
                            },
                        );
                        info!("PlaybackEngine: End of media reached (frame {}). Playback stopped by default.", max_f);
                        break;
                    }
                    current_frame_clone.store(actual_frame, Ordering::SeqCst);
                }

                if let Err(err) = event_sender_clone.send(
                    PlaybackTickEvent {
                        frame: actual_frame,
                        is_playing: true,
                    },
                ) {
                    warn!("Failed to emit playback event: {}", err);
                    break;
                }
            }
        });

        *handle_guard = Some(join_handle);
        info!("PlaybackEngine: Playback started @ {:.2} FPS", fps_val);
    }

    pub async fn pause(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.seek_requested.store(false, Ordering::SeqCst);

        let mut handle_guard = self.task_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }

        let frame = self.current_frame.load(Ordering::SeqCst);
        let _ = self.event_sender.send(
            PlaybackTickEvent {
                frame,
                is_playing: false,
            },
        );
        info!("PlaybackEngine: Paused at frame {}", frame);
    }

    pub async fn toggle(&self) {
        if self.is_playing.load(Ordering::SeqCst) {
            self.pause().await;
        } else {
            self.play().await;
        }
    }

    pub async fn seek(&self, mut target_frame: u64) {
        {
            let guard = self.loop_range.lock().await;
            if let Some((loop_in, loop_out)) = *guard {
                target_frame = target_frame.clamp(loop_in, loop_out);
            }
        }
        self.current_frame.store(target_frame, Ordering::SeqCst);
        self.seek_requested.store(true, Ordering::SeqCst);
        let is_play = self.is_playing.load(Ordering::SeqCst);

        let _ = self.event_sender.send(
            PlaybackTickEvent {
                frame: target_frame,
                is_playing: is_play,
            },
        );
    }

    pub async fn step_frame(&self, direction: i64) {
        if self.is_playing.load(Ordering::Relaxed) {
            self.pause().await;
        }

        let curr = self.current_frame.load(Ordering::Relaxed) as i64;
        let target = (curr + direction).max(0) as u64;
        self.seek(target).await;
    }
}