//! Conservative name-based hints. No shortcut resolution, file reads, or network access.
use crate::model::Item;

/// A hint is evidence for a proposal, never a claim that an unknown program was inspected.
pub fn hint(item: &Item) -> Option<&'static str> {
    if !item.evidence.product_name.is_empty() || !item.evidence.target_name.is_empty() {
        let mut identified = item.clone();
        identified.evidence = Default::default();
        identified.name = format!(
            "{} {} {} {}.exe",
            item.evidence.product_name,
            item.evidence.description,
            item.evidence.target_name.trim_end_matches(".exe"),
            item.evidence.shortcut_description
        );
        identified.extension = "exe".into();
        if let Some(hint) = hint(&identified) {
            return Some(hint);
        }
        if !item.evidence.target_extension.is_empty() && item.evidence.target_extension != "exe" {
            let category = crate::model::category_for(&item.evidence.target_extension);
            if category != "Other" && category != "Applications" {
                return Some(category);
            }
        }
    }
    if ["steam", "com.epicgames.launcher"].contains(&item.evidence.protocol.as_str()) {
        return Some("Games");
    }
    if item.kind == "folder" {
        return None;
    }
    if item.extension == "msi" {
        return Some("Installers");
    }
    if !["lnk", "url", "exe"].contains(&item.extension.as_str()) {
        return None;
    }
    let stem = item
        .name
        .rsplit_once('.')
        .map_or(item.name.as_str(), |(stem, _)| stem);
    let name = normalize(stem);
    // Match complete words/phrases, not substrings such as "edge" in "knowledge".
    RULES.iter().find_map(|(purpose, aliases)| {
        aliases
            .iter()
            .any(|alias| {
                name.match_indices(alias).any(|(start, _)| {
                    start > 0
                        && name.as_bytes()[start - 1] == b' '
                        && name.as_bytes().get(start + alias.len()) == Some(&b' ')
                })
            })
            .then_some(*purpose)
    })
}

fn normalize(name: &str) -> String {
    let mut result = String::from(" ");
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            result.push(character);
        } else if !result.ends_with(' ') {
            result.push(' ');
        }
    }
    if !result.ends_with(' ') {
        result.push(' ');
    }
    result
}

const RULES: &[(&str, &[&str])] = &[
    (
        "Browsers",
        &[
            "google chrome",
            "chrome",
            "firefox",
            "microsoft edge",
            "edge",
            "brave",
            "vivaldi",
            "opera",
            "yandex browser",
            "яндекс браузер",
        ],
    ),
    (
        "Development",
        &[
            "visual studio code",
            "vs code",
            "vscode",
            "visual studio",
            "intellij idea",
            "intellij",
            "pycharm",
            "webstorm",
            "rider",
            "android studio",
            "rustrover",
            "clion",
            "goland",
            "sublime text",
            "notepad",
            "github desktop",
            "git bash",
            "git gui",
            "docker desktop",
            "postman",
            "insomnia",
            "dbeaver",
            "datagrip",
            "unity",
            "unreal engine",
            "godot",
        ],
    ),
    (
        "Design & 3D",
        &[
            "blender",
            "autodesk fusion",
            "fusion 360",
            "autocad",
            "freecad",
            "solidworks",
            "rhino",
            "sketchup",
            "3ds max",
            "maya",
            "houdini",
            "zbrush",
            "cinema 4d",
            "figma",
            "inkscape",
            "illustrator",
            "coreldraw",
            "affinity designer",
        ],
    ),
    (
        "Photo editing",
        &[
            "photoshop",
            "lightroom",
            "gimp",
            "krita",
            "paint net",
            "paint",
            "affinity photo",
            "darktable",
            "rawtherapee",
        ],
    ),
    (
        "Video tools",
        &[
            "davinci resolve",
            "premiere pro",
            "after effects",
            "capcut",
            "shotcut",
            "kdenlive",
            "handbrake",
            "obs studio",
            "obs",
            "vlc",
            "mpv",
            "media player classic",
        ],
    ),
    (
        "Audio & music",
        &[
            "audacity",
            "reaper",
            "fl studio",
            "ableton live",
            "cubase",
            "studio one",
            "lmms",
            "spotify",
            "foobar2000",
            "aimp",
            "itunes",
            "apple music",
        ],
    ),
    (
        "Games",
        &[
            "steam",
            "epic games",
            "gog galaxy",
            "battle net",
            "ubisoft connect",
            "ea app",
            "riot client",
            "minecraft",
            "roblox",
            "fortnite",
            "dota 2",
            "counter strike 2",
            "baldur s gate 3",
            "cyberpunk 2077",
            "terraria",
            "stardew valley",
        ],
    ),
    (
        "Communication",
        &[
            "telegram",
            "discord",
            "slack",
            "whatsapp",
            "signal",
            "skype",
            "zoom",
            "microsoft teams",
            "teams",
            "viber",
            "thunderbird",
            "outlook",
        ],
    ),
    (
        "Office & writing",
        &[
            "microsoft word",
            "word",
            "excel",
            "powerpoint",
            "onenote",
            "libreoffice",
            "openoffice",
            "onlyoffice",
            "obsidian",
            "notion",
            "joplin",
            "evernote",
            "typora",
            "acrobat",
            "foxit",
            "sumatrapdf",
            "sumatra pdf",
        ],
    ),
    (
        "File tools",
        &[
            "7 zip",
            "7zip",
            "winrar",
            "peazip",
            "bandizip",
            "total commander",
            "double commander",
            "freecommander",
            "everything",
            "wiztree",
            "windirstat",
            "spacesniffer",
            "filezilla",
            "winscp",
            "qbittorrent",
            "transmission",
            "syncthing",
        ],
    ),
    (
        "Remote access",
        &[
            "anydesk",
            "teamviewer",
            "rustdesk",
            "parsec",
            "remote desktop",
            "удаленный рабочий стол",
            "удалённый рабочий стол",
        ],
    ),
    (
        "Security",
        &[
            "bitwarden",
            "keepass",
            "keepassxc",
            "1password",
            "veracrypt",
            "malwarebytes",
            "windows security",
            "безопасность windows",
        ],
    ),
    (
        "System tools",
        &[
            "powertoys",
            "task manager",
            "диспетчер задач",
            "control panel",
            "панель управления",
            "device manager",
            "диспетчер устройств",
            "registry editor",
            "редактор реестра",
            "resource monitor",
            "монитор ресурсов",
            "process explorer",
            "process monitor",
            "autoruns",
            "hwinfo",
            "hwmonitor",
            "cpu z",
            "gpu z",
            "rufus",
            "balenaetcher",
            "crystaldiskinfo",
            "crystaldiskmark",
            "windows terminal",
            "powershell",
            "command prompt",
        ],
    ),
    (
        "AI tools",
        &[
            "chatgpt",
            "claude",
            "codex",
            "lm studio",
            "ollama",
            "jan",
            "comfyui",
            "comfy ui",
            "a1111",
            "fooocus",
            "invokeai",
            "open webui",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_handle_shortcut_suffixes_punctuation_versions_and_localized_names() {
        for (name, expected) in [
            ("Mozilla Firefox (2).lnk", "Browsers"),
            ("Visual Studio Code - Shortcut.lnk", "Development"),
            ("Blender 4.5.lnk", "Design & 3D"),
            ("7-Zip File Manager.lnk", "File tools"),
            ("Диспетчер задач.lnk", "System tools"),
            ("Unknown product.msi", "Installers"),
        ] {
            assert_eq!(
                hint(&Item::from_path(1, name.into(), false)),
                Some(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn unknown_names_documents_and_similar_substrings_do_not_get_invented_purposes() {
        for (name, directory) in [
            ("Knowledge.lnk", false),
            ("Paintings.lnk", false),
            ("Discord notes.pdf", false),
            ("Blender", true),
            ("Personal launcher.lnk", false),
        ] {
            assert_eq!(
                hint(&Item::from_path(1, name.into(), directory)),
                None,
                "{name}"
            );
        }
    }
}
