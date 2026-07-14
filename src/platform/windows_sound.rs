//! Windows sound playback: plays a synthesized WAV buffer through winmm
//! `PlaySound` (SND_MEMORY | SND_ASYNC, no audio assets, no extra
//! dependencies). Shared by both the native and `--features portable`
//! Windows backends — each registers [`play`] as `crate::sound`'s playback
//! hook via `sound::set_player` at startup.

pub fn play(data: &'static [u8]) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    unsafe {
        PlaySoundW(
            data.as_ptr() as *const u16,
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}
