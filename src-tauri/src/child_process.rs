use std::process::Command;

/// 为不应显示控制台窗口的后台子进程应用平台创建参数。
pub(crate) fn configure_background_command(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        command.creation_flags(background_creation_flags());
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

/// 返回后台子进程在 Windows 上应使用的创建参数。
#[cfg(windows)]
pub(crate) fn background_creation_flags() -> u32 {
    windows_sys::Win32::System::Threading::CREATE_NO_WINDOW
}

#[cfg(all(test, windows))]
mod tests {
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    use super::background_creation_flags;

    #[test]
    fn background_processes_do_not_create_console_windows() {
        assert_eq!(background_creation_flags(), CREATE_NO_WINDOW);
    }
}
