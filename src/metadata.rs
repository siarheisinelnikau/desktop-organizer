//! Read-only program evidence. Paths and argument strings never enter AI metadata.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Evidence {
    pub target_name: String,
    pub target_extension: String,
    pub product_name: String,
    pub description: String,
    pub company: String,
    pub shortcut_description: String,
    pub website_host: String,
    pub protocol: String,
    /// Local identity only; deliberately excluded from the AI allowlist.
    pub identity: String,
    pub status: String,
}
impl Evidence {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
    pub fn valid(&self) -> bool {
        [
            &self.target_name,
            &self.target_extension,
            &self.product_name,
            &self.description,
            &self.company,
            &self.shortcut_description,
            &self.website_host,
            &self.protocol,
            &self.status,
        ]
        .iter()
        .all(|s| s.chars().count() <= 256 && !s.chars().any(char::is_control))
            && self.identity.len() <= 64
    }
}
pub fn identity(value: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(value.to_lowercase().replace('/', "\\").as_bytes())
    )
}
pub fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect::<String>()
        .trim()
        .to_owned()
}
pub fn inspect(item: &crate::model::Item) -> Evidence {
    if item.kind == "folder" || !["lnk", "url", "exe"].contains(&item.extension.as_str()) {
        return Evidence::default();
    }
    let mut evidence = Evidence {
        status: "Target unavailable".into(),
        ..Default::default()
    };
    if crate::operations::checked_path(&item.path, true).is_err() {
        return evidence;
    }
    if item.extension == "url" {
        if let Ok(text) = read_shortcut(&item.path) {
            evidence = internet_shortcut(&text);
        }
        return evidence;
    }
    #[cfg(windows)]
    {
        let target = if item.extension == "lnk" {
            shell_target(&item.path, &mut evidence)
        } else {
            Some(item.path.clone())
        };
        if let Some(target) = target {
            if evidence.identity.is_empty() {
                evidence.identity = identity(&target.to_string_lossy());
            }
            evidence.target_name = clean(&target.file_name().unwrap_or_default().to_string_lossy());
            evidence.target_extension = clean(
                &target
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            );
            evidence.status = "Target identified; description unavailable".into();
            // Do not probe network drives, devices, reparse points or cloud placeholders.
            if evidence.target_extension == "exe"
                && local_fixed_file(&target)
                && let Some((product, description, company)) = version_strings(&target)
            {
                evidence.product_name = product;
                evidence.description = description;
                evidence.company = company;
                evidence.status = "Program description read".into();
            }
        }
    }
    evidence
}
fn read_shortcut(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 64 * 1024 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        Ok(String::from_utf16_lossy(
            &bytes[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes(*b))
                .collect::<Vec<_>>(),
        ))
    } else {
        Ok(String::from_utf8_lossy(&bytes)
            .trim_start_matches('\u{feff}')
            .to_owned())
    }
}
fn internet_shortcut(text: &str) -> Evidence {
    let mut in_section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[InternetShortcut]");
        }
        if in_section
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("URL")
            && let Ok(url) = url::Url::parse(value.trim())
        {
            return Evidence {
                identity: identity(value.trim()),
                protocol: clean(url.scheme()),
                website_host: if matches!(url.scheme(), "http" | "https") {
                    clean(url.host_str().unwrap_or_default())
                } else {
                    String::new()
                },
                status: "Shortcut destination read".into(),
                ..Default::default()
            };
        }
    }
    Evidence {
        status: "Shortcut destination unavailable".into(),
        ..Default::default()
    }
}

#[cfg(windows)]
fn shell_target(path: &Path, evidence: &mut Evidence) -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{System::Com::*, UI::Shell::*},
        core::{Interface, PCWSTR},
    };
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let _apartment = Apartment(initialized);
        // Read the saved link, without Resolve, target launch, tracking or argument access.
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        persist.Load(PCWSTR(path.as_ptr()), STGM_READ).ok()?;
        let mut description = [0u16; 512];
        if link.GetDescription(&mut description).is_ok() {
            evidence.shortcut_description = clean(&utf16(&description));
        }
        let mut target = [0u16; 32768];
        link.GetPath(&mut target, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
            .ok()?;
        let target = utf16(&target);
        let mut arguments = [0u16; 4096];
        if !target.is_empty() && link.GetArguments(&mut arguments).is_ok() {
            // Arguments distinguish game IDs and browser profiles locally. Only their
            // hash is retained; raw arguments are never stored, displayed or shared.
            let arguments = utf16(&arguments);
            evidence.identity = if arguments.is_empty() {
                identity(&target)
            } else {
                identity(&format!("{}\n{}", target, arguments))
            };
        }
        if target.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(target))
        }
    }
}
#[cfg(windows)]
fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}
#[cfg(windows)]
fn local_fixed_file(path: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'\\' {
        return false;
    }
    let root: Vec<u16> = text[..3].encode_utf16().chain(Some(0)).collect();
    (unsafe { GetDriveTypeW(root.as_ptr()) == 3 })
        && crate::operations::checked_path(path, true).is_ok()
        && path.is_file()
}
#[cfg(windows)]
fn version_strings(path: &Path) -> Option<(String, String, String)> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::Storage::FileSystem::*;
    unsafe {
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let size = GetFileVersionInfoSizeW(path.as_ptr(), null_mut());
        if size == 0 || size > 1024 * 1024 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let query = |key: &str| -> Option<(*const u8, usize)> {
            let key: Vec<_> = key.encode_utf16().chain(Some(0)).collect();
            let mut out: *mut c_void = null_mut();
            let mut length = 0;
            if VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut out, &mut length) == 0
                || out.is_null()
            {
                return None;
            }
            let offset = (out as usize).checked_sub(data.as_ptr() as usize)?;
            (offset < data.len()).then_some((out.cast(), length as usize))
        };
        let mut translations = vec![(0x0409u16, 0x04b0u16), (0x0409, 0x04e4)];
        if let Some((pointer, len)) = query("\\VarFileInfo\\Translation") {
            let offset = pointer as usize - data.as_ptr() as usize;
            if len <= data.len() - offset {
                for pair in std::slice::from_raw_parts(pointer, len)
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .take(16)
                {
                    translations.insert(
                        0,
                        (
                            u16::from_le_bytes([pair[0], pair[1]]),
                            u16::from_le_bytes([pair[2], pair[3]]),
                        ),
                    );
                }
            }
        }
        let string = |name: &str| -> String {
            for (language, page) in &translations {
                if let Some((pointer, length)) = query(&format!(
                    "\\StringFileInfo\\{language:04x}{page:04x}\\{name}"
                )) {
                    let offset = pointer as usize - data.as_ptr() as usize;
                    if length <= (data.len() - offset) / 2 && (pointer as usize).is_multiple_of(2) {
                        let value = clean(&utf16(std::slice::from_raw_parts(
                            pointer.cast::<u16>(),
                            length,
                        )));
                        if !value.is_empty() {
                            return value;
                        }
                    }
                }
            }
            String::new()
        };
        let result = (
            string("ProductName"),
            string("FileDescription"),
            string("CompanyName"),
        );
        if result.0.is_empty() && result.1.is_empty() && result.2.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn internet_destination_exposes_host_and_protocol_without_credentials_or_query() {
        let evidence = internet_shortcut(
            "[InternetShortcut]\r\nURL=https://user:secret@example.test/private/report?token=hidden\r\n",
        );
        assert_eq!(evidence.website_host, "example.test");
        assert_eq!(evidence.protocol, "https");
        let text = serde_json::to_string(&evidence).unwrap();
        assert!(!text.contains("secret") && !text.contains("hidden") && !text.contains("private"));
        assert_eq!(evidence.identity.len(), 64);
        assert_eq!(
            internet_shortcut("[InternetShortcut]\nURL=steam://rungameid/123").protocol,
            "steam"
        );
    }
    #[cfg(windows)]
    #[test]
    fn saved_shell_link_is_read_without_opening_target_and_identity_survives_rename() {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::{System::Com::*, UI::Shell::*},
            core::{Interface, PCWSTR},
        };
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Unclear.lnk");
        let target = root.path().join("Blender.exe");
        std::fs::write(&target, b"not executable").unwrap();
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let wide_target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
            link.SetPath(PCWSTR(wide_target.as_ptr())).unwrap();
            link.SetDescription(windows::core::w!("3D modelling fixture"))
                .unwrap();
            let persist: IPersistFile = link.cast().unwrap();
            let wide_path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            persist.Save(PCWSTR(wide_path.as_ptr()), true).unwrap();
            drop(persist);
            drop(link);
            if initialized {
                CoUninitialize();
            }
        }
        let first = inspect(&crate::model::Item::from_path(1, path.clone(), false));
        assert_eq!(first.target_name, "Blender.exe");
        assert_eq!(first.shortcut_description, "3D modelling fixture");
        let renamed = root.path().join("Renamed.lnk");
        std::fs::rename(path, &renamed).unwrap();
        assert_eq!(
            inspect(&crate::model::Item::from_path(1, renamed, false)).identity,
            first.identity
        );
        assert_eq!(std::fs::read(target).unwrap(), b"not executable");
    }
    #[cfg(windows)]
    #[test]
    fn executable_version_resource_is_read_as_data() {
        let path = std::env::current_exe().unwrap();
        let (product, description, _) =
            version_strings(&path).expect("Test binary embeds this product's version resource");
        assert!(!product.is_empty() || !description.is_empty());
    }
}
