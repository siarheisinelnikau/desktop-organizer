//! Explicit user choices, keyed locally by target identity and source reference.
use crate::{grouping::Suggestion, model::Item};
use serde::{Deserialize, Serialize};
pub const MAX_CORRECTIONS: usize = 4000;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Correction {
    pub key: String,
    pub group: String,
}
pub fn preferred<'a>(rules: &'a [Correction], item: &Item) -> Option<&'a str> {
    let target = &item.evidence.identity;
    let reference = crate::metadata::identity(&item.path.to_string_lossy());
    rules
        .iter()
        .rev()
        .find(|rule| (!target.is_empty() && rule.key == *target) || rule.key == reference)
        .map(|r| r.group.as_str())
}
pub fn remember(rules: &mut Vec<Correction>, items: &[Item], group: &str) {
    if group.trim().is_empty() || group.chars().count() > 60 || group.chars().any(char::is_control)
    {
        return;
    }
    for item in items {
        for key in [
            item.evidence.identity.clone(),
            crate::metadata::identity(&item.path.to_string_lossy()),
        ] {
            if key.is_empty() {
                continue;
            }
            rules.retain(|rule| rule.key != key);
            rules.push(Correction {
                key,
                group: group.to_owned(),
            });
        }
    }
    if rules.len() > MAX_CORRECTIONS {
        rules.drain(..rules.len() - MAX_CORRECTIONS);
    }
}
pub fn apply(groups: Vec<Suggestion>, items: &[Item], rules: &[Correction]) -> Vec<Suggestion> {
    let mut result: Vec<Suggestion> = Vec::new();
    for group in groups {
        for id in group.item_ids {
            let name = items
                .iter()
                .find(|i| i.id == id)
                .and_then(|i| preferred(rules, i))
                .unwrap_or(&group.name);
            if let Some(existing) = result
                .iter_mut()
                .find(|g| g.name.to_lowercase() == name.to_lowercase())
            {
                existing.item_ids.push(id);
            } else {
                result.push(Suggestion {
                    name: name.to_owned(),
                    item_ids: vec![id],
                });
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_choice_survives_restart_rename_and_overrules_future_ai_proposals() {
        let mut item =
            Item::from_path(1, std::env::current_dir().unwrap().join("First.lnk"), false);
        item.evidence.identity = crate::metadata::identity("C:/Program Files/Game/game.exe");
        let mut rules = vec![];
        remember(&mut rules, &[item.clone()], "Games for evenings");
        let rules: Vec<Correction> =
            serde_json::from_str(&serde_json::to_string(&rules).unwrap()).unwrap();
        item.path.set_file_name("Renamed.lnk");
        let groups = apply(
            vec![Suggestion {
                name: "Applications".into(),
                item_ids: vec![1],
            }],
            &[item],
            &rules,
        );
        assert_eq!(groups[0].name, "Games for evenings");
        assert!(
            !serde_json::to_string(&rules)
                .unwrap()
                .contains("Program Files")
        );
    }
}
