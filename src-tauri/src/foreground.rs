fn provider_for_executable(path: &str) -> Option<&'static str> {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let mut components = normalized.rsplit('/');
    match components.next()? {
        "codex.exe" => Some("codex"),
        "chatgpt.exe" => {
            // The Codex MSIX currently ships app/ChatGPT.exe. Check its package
            // directory and publisher as well, so ordinary ChatGPT stays unrelated.
            let app_dir = components.next()?;
            let package = components.next()?;
            let package_root = components.next()?;
            (app_dir == "app"
                && package_root == "windowsapps"
                && package.starts_with("openai.codex_")
                && package.ends_with("__2p2nqsd0c76g0"))
            .then_some("codex")
        }
        "workbuddy.exe" => Some("workbuddy"),
        "trae.exe" | "trae cn.exe" => Some("trae"),
        "atomcode.exe" => Some("atomcode"),
        _ => None,
    }
}

/// Only return a provider key; window titles and process paths stay on the native side.
#[tauri::command]
pub(crate) fn get_foreground_provider() -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        use windows::{
            core::PWSTR,
            Win32::{
                Foundation::CloseHandle,
                System::Threading::{
                    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
                UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
            },
        };

        // A foreground window may disappear or be inaccessible during a focus change.
        // In either case leave the displayed provider alone.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 || pid == std::process::id() {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut path = vec![0u16; 32768];
            let mut length = path.len() as u32;
            let result = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(process);
            result.ok()?;
            // Discard a stale observation if focus changed while resolving the process.
            if GetForegroundWindow() != hwnd {
                return None;
            }
            provider_for_executable(&String::from_utf16_lossy(&path[..length as usize]))
        }
    }
    #[cfg(not(target_os = "windows"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_installed_codex_packaged_as_chatgpt() {
        assert_eq!(
            provider_for_executable(
                r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe"
            ),
            Some("codex")
        );
        assert_eq!(
            provider_for_executable(
                r"D:\WINDOWSAPPS\OPENAI.CODEX_27.1.0.0_ARM64__2P2NQSD0C76G0\APP\CHATGPT.EXE"
            ),
            Some("codex")
        );
    }

    #[test]
    fn does_not_treat_regular_chatgpt_or_lookalike_paths_as_codex() {
        for path in [
            r"C:\Apps\ChatGPT.exe",
            r"C:\Program Files\WindowsApps\OpenAI.ChatGPT_26.1.0.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe",
            r"C:\Projects\OpenAI.Codex_26.1.0.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe",
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1.0.0_x64__otherpublisher\app\ChatGPT.exe",
        ] {
            assert_eq!(provider_for_executable(path), None, "{path}");
        }
    }

    #[test]
    fn recognizes_supported_apps_case_insensitively() {
        for (path, expected) in [
            (r"C:\Apps\Codex\Codex.exe", "codex"),
            (r"C:\Apps\WorkBuddy.exe", "workbuddy"),
            (r"C:\Apps\TRAE.exe", "trae"),
            (r"C:\Apps\TRAE CN.exe", "trae"),
            (r"C:\Apps\AtomCode.EXE", "atomcode"),
        ] {
            assert_eq!(provider_for_executable(path), Some(expected), "{path}");
        }
    }

    #[test]
    fn ignores_unrelated_apps_and_provider_names_in_directories() {
        for path in [
            r"C:\Apps\codexbar-windows.exe",
            r"C:\Codex\Code.exe",
            r"C:\Apps\WindowsTerminal.exe",
            r"C:\Apps\chrome.exe",
            r"C:\Apps\not-codex.exe",
            "",
        ] {
            assert_eq!(provider_for_executable(path), None, "{path}");
        }
    }
}
