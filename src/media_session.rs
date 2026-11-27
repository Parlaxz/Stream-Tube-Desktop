//! Windows System Media Transport Controls: media keys on keyboards, headsets
//! and the Windows media overlay control playback, also while the app runs in
//! the background. Other platforms get inert stubs.

#[cfg(not(windows))]
pub fn init(_window: &gpui_kit::Window, _cx: &mut gpui_kit::App) {}

#[cfg(not(windows))]
pub fn now_playing(_title: &str, _artist: &str, _playing: bool) {}

#[cfg(not(windows))]
pub fn set_playing(_playing: bool) {}

#[cfg(not(windows))]
pub fn clear() {}

#[cfg(windows)]
pub fn init(window: &gpui_kit::Window, cx: &mut gpui_kit::App) {
    win::init(window, cx);
}

#[cfg(windows)]
pub fn now_playing(title: &str, artist: &str, playing: bool) {
    win::now_playing(title, artist, playing);
}

#[cfg(windows)]
pub fn set_playing(playing: bool) {
    win::set_playing(playing);
}

#[cfg(windows)]
pub fn clear() {
    win::clear();
}

#[cfg(windows)]
mod win {
    use futures::channel::mpsc::{UnboundedReceiver, unbounded};
    use std::sync::OnceLock;
    use windows::Foundation::TypedEventHandler;
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    };
    use windows::Win32::Foundation::HWND;
    use windows::core::{GUID, HSTRING, IInspectable_Vtbl, IUnknown, Interface, Type, factory};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MediaCommand {
        Play,
        Pause,
        Stop,
    }

    static CONTROLS: OnceLock<SystemMediaTransportControls> = OnceLock::new();

    /// `ISystemMediaTransportControlsInterop` from windows.media.h. The windows
    /// crate does not generate it, and it is the only way for a desktop app to
    /// get the controls for its window (`GetForCurrentView` needs a CoreWindow).
    #[repr(transparent)]
    #[derive(Clone, PartialEq, Eq)]
    struct Interop(IUnknown);

    #[repr(C)]
    struct InteropVtbl {
        base: IInspectable_Vtbl,
        get_for_window: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            HWND,
            *const GUID,
            *mut *mut core::ffi::c_void,
        ) -> windows::core::HRESULT,
    }

    unsafe impl Interface for Interop {
        type Vtable = InteropVtbl;
        const IID: GUID = GUID::from_u128(0xddb0472d_c911_4a1f_86d9_dc3d71a95f5a);
    }

    pub(super) fn init(window: &gpui_kit::Window, cx: &mut gpui_kit::App) {
        if CONTROLS.get().is_some() {
            return;
        }
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };

        let interop: Interop = match factory::<SystemMediaTransportControls, Interop>() {
            Ok(interop) => interop,
            Err(_) => return,
        };
        let mut raw = std::ptr::null_mut();
        let hr = unsafe {
            (Interface::vtable(&interop).get_for_window)(
                Interface::as_raw(&interop),
                HWND(handle.hwnd.get() as *mut core::ffi::c_void),
                &SystemMediaTransportControls::IID,
                &mut raw,
            )
        };
        if hr.is_err() {
            return;
        }
        let controls: SystemMediaTransportControls = match unsafe { Type::from_abi(raw) } {
            Ok(controls) => controls,
            Err(_) => return,
        };

        let _ = controls.SetIsEnabled(true);
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsStopEnabled(true);
        let _ = controls.SetIsNextEnabled(false);
        let _ = controls.SetIsPreviousEnabled(false);
        let _ = controls.SetIsFastForwardEnabled(false);
        let _ = controls.SetIsRewindEnabled(false);
        let _ = controls.SetIsRecordEnabled(false);
        let _ = controls.SetIsChannelUpEnabled(false);
        let _ = controls.SetIsChannelDownEnabled(false);
        let _ = controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);

        let (tx, rx) = unbounded();
        let handler: TypedEventHandler<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        > =
            TypedEventHandler::new(
                move |_: windows::core::Ref<SystemMediaTransportControls>,
                      args: windows::core::Ref<
                    SystemMediaTransportControlsButtonPressedEventArgs,
                >| {
                    if let Some(cmd) = args
                        .ok()
                        .ok()
                        .and_then(|args| args.Button().ok())
                        .and_then(command)
                    {
                        let _ = tx.unbounded_send(cmd);
                    }
                    Ok(())
                },
            );
        let _ = controls.ButtonPressed(&handler);

        if CONTROLS.set(controls).is_err() {
            return;
        }
        run(rx, cx);
    }

    fn command(button: SystemMediaTransportControlsButton) -> Option<MediaCommand> {
        if button == SystemMediaTransportControlsButton::Play {
            Some(MediaCommand::Play)
        } else if button == SystemMediaTransportControlsButton::Pause {
            Some(MediaCommand::Pause)
        } else if button == SystemMediaTransportControlsButton::Stop {
            Some(MediaCommand::Stop)
        } else {
            None
        }
    }

    fn run(mut rx: UnboundedReceiver<MediaCommand>, cx: &mut gpui_kit::App) {
        cx.spawn(async move |cx| {
            use futures::StreamExt;
            while let Some(cmd) = rx.next().await {
                let _ = cx.update(|cx| apply(cmd, cx));
            }
        })
        .detach();
    }

    fn apply(cmd: MediaCommand, cx: &mut gpui_kit::App) {
        let p = crate::player::controller::player(cx);
        p.update(cx, |p, cx| {
            if p.video.is_none() {
                return;
            }
            match cmd {
                MediaCommand::Play => {
                    if p.eof {
                        p.toggle_pause(cx);
                    } else if p.paused {
                        p.set_paused(false, cx);
                    }
                }
                MediaCommand::Pause => {
                    if !p.paused {
                        p.set_paused(true, cx);
                    }
                }
                MediaCommand::Stop => p.stop(cx),
            }
        });
    }

    pub(super) fn now_playing(title: &str, artist: &str, playing: bool) {
        let Some(controls) = CONTROLS.get() else {
            return;
        };
        if let Ok(updater) = controls.DisplayUpdater() {
            let _ = updater.SetType(MediaPlaybackType::Video);
            if let Ok(props) = updater.VideoProperties() {
                let _ = props.SetTitle(&HSTRING::from(title));
                let _ = props.SetSubtitle(&HSTRING::from(artist));
            }
            let _ = updater.Update();
        }
        set_playing(playing);
    }

    pub(super) fn set_playing(playing: bool) {
        if let Some(controls) = CONTROLS.get() {
            let status = if playing {
                MediaPlaybackStatus::Playing
            } else {
                MediaPlaybackStatus::Paused
            };
            let _ = controls.SetPlaybackStatus(status);
        }
    }

    pub(super) fn clear() {
        let Some(controls) = CONTROLS.get() else {
            return;
        };
        let _ = controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
        if let Ok(updater) = controls.DisplayUpdater() {
            let _ = updater.ClearAll();
            let _ = updater.Update();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn transport_buttons_map_to_commands() {
            assert_eq!(
                command(SystemMediaTransportControlsButton::Play),
                Some(MediaCommand::Play)
            );
            assert_eq!(
                command(SystemMediaTransportControlsButton::Pause),
                Some(MediaCommand::Pause)
            );
            assert_eq!(
                command(SystemMediaTransportControlsButton::Stop),
                Some(MediaCommand::Stop)
            );
            assert_eq!(command(SystemMediaTransportControlsButton::Next), None);
        }
    }
}
