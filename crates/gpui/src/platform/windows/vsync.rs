use std::{
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError},
        LazyLock,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use util::ResultExt;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DWM_TIMING_INFO, DwmFlush, DwmGetCompositionTimingInfo},
    System::Performance::QueryPerformanceFrequency,
};

static QPC_TICKS_PER_SECOND: LazyLock<u64> = LazyLock::new(|| {
    let mut frequency = 0;
    // On systems that run Windows XP or later, the function will always succeed and
    // will thus never return zero.
    unsafe { QueryPerformanceFrequency(&mut frequency).unwrap() };
    frequency as u64
});

const VSYNC_INTERVAL_THRESHOLD: Duration = Duration::from_millis(1);
const DEFAULT_VSYNC_INTERVAL: Duration = Duration::from_micros(16_666); // ~60Hz
/// Maximum time the vsync loop will wait on `DwmFlush` before falling back to a
/// fixed-rate sleep. Under certain driver/compositor conditions (notably with
/// REALTIME GPU scheduling) `DwmFlush` can block indefinitely; without this
/// guard the entire UI freezes because no frames are scheduled.
const DWM_FLUSH_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) struct VSyncProvider {
    interval: Duration,
    flush_req: Sender<()>,
    flush_done: Receiver<bool>,
    /// True while the worker is still stuck inside a previous `DwmFlush` that
    /// never came back within the timeout. We don't issue further requests
    /// until the worker drains it; the vsync loop falls back to `Sleep` in the
    /// meantime so the UI keeps painting.
    worker_busy: bool,
    fallback_logged: bool,
}

impl VSyncProvider {
    pub(crate) fn new() -> Self {
        let interval = get_dwm_interval()
            .context("Failed to get DWM interval")
            .log_err()
            .unwrap_or(DEFAULT_VSYNC_INTERVAL);

        // Offload `DwmFlush` to a dedicated worker thread. If it hangs, only
        // that worker is stuck — the vsync loop times out and keeps the UI
        // alive on a fixed-rate sleep instead of freezing the whole app.
        let (flush_req, req_rx) = channel::<()>();
        let (done_tx, flush_done) = channel::<bool>();
        let _ = thread::Builder::new()
            .name("DwmFlushWorker".to_owned())
            .spawn(move || {
                while req_rx.recv().is_ok() {
                    let ok = unsafe { DwmFlush().is_ok() };
                    if done_tx.send(ok).is_err() {
                        break;
                    }
                }
            });

        Self {
            interval,
            flush_req,
            flush_done,
            worker_busy: false,
            fallback_logged: false,
        }
    }

    pub(crate) fn wait_for_vsync(&mut self) {
        let vsync_start = Instant::now();
        let wait_succeeded = self.dwm_flush_with_timeout();
        let elapsed = vsync_start.elapsed();
        // DwmFlush and DCompositionWaitForCompositorClock returns very early
        // instead of waiting until vblank when the monitor goes to sleep or is
        // unplugged (nothing to present due to desktop occlusion). We use 1ms as
        // a threshold for the duration of the wait functions and fallback to
        // Sleep() if it returns before that. This could happen during normal
        // operation for the first call after the vsync thread becomes non-idle,
        // but it shouldn't happen often.
        if !wait_succeeded || elapsed < VSYNC_INTERVAL_THRESHOLD {
            log::trace!("VSyncProvider::wait_for_vsync() took less time than expected");
            std::thread::sleep(self.interval);
        }
    }

    /// Returns true if `DwmFlush` completed successfully within the timeout.
    /// Returns false if it timed out or the worker is still busy from a
    /// previously hung flush — the caller falls back to a fixed-rate sleep.
    fn dwm_flush_with_timeout(&mut self) -> bool {
        // If a previous flush hung past the timeout, the worker may still be
        // blocked. Try to reclaim it non-blockingly before issuing a new
        // request. If the worker eventually returns we resume DWM-driven sync;
        // otherwise we stay in fallback mode without piling up requests.
        if self.worker_busy {
            match self.flush_done.try_recv() {
                Ok(_) => {
                    self.worker_busy = false;
                    if self.fallback_logged {
                        log::warn!(
                            "[VSyncProvider] DwmFlush recovered; resuming DWM-driven vsync"
                        );
                        self.fallback_logged = false;
                    }
                }
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return false,
            }
        }

        if self.flush_req.send(()).is_err() {
            return false;
        }
        match self.flush_done.recv_timeout(DWM_FLUSH_TIMEOUT) {
            Ok(ok) => ok,
            Err(RecvTimeoutError::Timeout) => {
                self.worker_busy = true;
                if !self.fallback_logged {
                    log::warn!(
                        "[VSyncProvider] DwmFlush did not return within {:?}; falling back \
                         to fixed-rate vsync until it recovers",
                        DWM_FLUSH_TIMEOUT
                    );
                    self.fallback_logged = true;
                }
                false
            }
            Err(RecvTimeoutError::Disconnected) => false,
        }
    }
}

fn get_dwm_interval() -> Result<Duration> {
    let mut timing_info = DWM_TIMING_INFO {
        cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing_info) }?;
    let interval = retrieve_duration(timing_info.qpcRefreshPeriod, *QPC_TICKS_PER_SECOND);
    // Check for interval values that are impossibly low. A 29 microsecond
    // interval was seen (from a qpcRefreshPeriod of 60).
    if interval < VSYNC_INTERVAL_THRESHOLD {
        Ok(retrieve_duration(
            timing_info.rateRefresh.uiDenominator as u64,
            timing_info.rateRefresh.uiNumerator as u64,
        ))
    } else {
        Ok(interval)
    }
}

#[inline]
fn retrieve_duration(counts: u64, ticks_per_second: u64) -> Duration {
    let ticks_per_microsecond = ticks_per_second / 1_000_000;
    Duration::from_micros(counts / ticks_per_microsecond)
}
