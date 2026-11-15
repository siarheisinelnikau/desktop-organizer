use crate::model::{Item, Layout, MAX_GROUPS};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub name: String,
    pub item_ids: Vec<u64>,
}

pub fn offline(items: &[Item]) -> Vec<Suggestion> {
    let mut groups = BTreeMap::<String, Vec<u64>>::new();
    for item in items {
        groups
            .entry(proposed_category(item).to_owned())
            .or_default()
            .push(item.id);
    }
    groups
        .into_iter()
        .map(|(name, item_ids)| Suggestion { name, item_ids })
        .collect()
}

pub fn proposed_category(item: &Item) -> &str {
    crate::purpose::hint(item).unwrap_or_else(|| {
        if item.category == "Applications" {
            "Needs review"
        } else {
            &item.category
        }
    })
}

pub fn validate(suggestions: &[Suggestion], items: &[Item]) -> Result<()> {
    ensure!(
        !suggestions.is_empty() && suggestions.len() <= MAX_GROUPS,
        "Expected 1 to {MAX_GROUPS} groups"
    );
    let allowed: HashSet<_> = items.iter().map(|i| i.id).collect();
    let mut used = HashSet::new();
    let mut names = HashSet::new();
    for group in suggestions {
        ensure!(
            !group.name.trim().is_empty()
                && group.name.chars().count() <= 60
                && !group.name.chars().any(char::is_control),
            "Invalid group name"
        );
        ensure!(
            names.insert(group.name.to_lowercase()),
            "Duplicate group name"
        );
        ensure!(!group.item_ids.is_empty(), "Empty group");
        for id in &group.item_ids {
            ensure!(
                allowed.contains(id) && used.insert(*id),
                "Unknown or duplicate item ID"
            );
        }
    }
    ensure!(
        used == allowed,
        "Every selected item must appear exactly once"
    );
    Ok(())
}

pub fn apply(layout: &mut Layout, suggestions: &[Suggestion]) -> Result<()> {
    validate(suggestions, &layout.items)?;
    let mut revised = layout.clone();
    revised.groups.clear();
    for proposal in suggestions {
        let id = revised
            .add_group(&proposal.name)
            .expect("validated group limit");
        revised
            .groups
            .iter_mut()
            .find(|g| g.id == id)
            .unwrap()
            .items = proposal.item_ids.clone();
    }
    *layout = revised;
    Ok(())
}

pub fn apply_selected(
    layout: &mut Layout,
    suggestions: &[Suggestion],
    selected: &[Item],
) -> Result<()> {
    validate(suggestions, selected)?;
    ensure!(
        selected.iter().all(|item| layout.items.contains(item)),
        "Items changed since the suggestion; request a fresh preview"
    );
    let selected_ids: HashSet<_> = selected.iter().map(|i| i.id).collect();
    let mut revised = layout.clone();
    let placements: BTreeMap<_, _> = layout
        .groups
        .iter()
        .map(|g| {
            (
                g.name.to_lowercase(),
                (g.placement.clone(), g.collapsed, g.on_screen),
            )
        })
        .collect();
    for group in &mut revised.groups {
        group.items.retain(|id| !selected_ids.contains(id));
    }
    revised.groups.retain(|g| !g.items.is_empty());
    for proposal in suggestions {
        if let Some(group) = revised
            .groups
            .iter_mut()
            .find(|g| g.name.eq_ignore_ascii_case(&proposal.name))
        {
            group.items.extend(&proposal.item_ids);
        } else {
            let id = revised
                .add_group(&proposal.name)
                .ok_or_else(|| anyhow::anyhow!("Too many groups; merge some groups first"))?;
            let added = revised.groups.iter_mut().find(|g| g.id == id).unwrap();
            added.items = proposal.item_ids.clone();
            if let Some((placement, collapsed, on_screen)) =
                placements.get(&proposal.name.to_lowercase())
            {
                added.placement = placement.clone();
                added.collapsed = *collapsed;
                added.on_screen = *on_screen;
            }
        }
    }
    revised.validate()?;
    *layout = revised;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn stable_offline_grouping_and_exact_coverage() {
        let items = vec![
            Item::from_path(1, PathBuf::from("a.pdf"), false),
            Item::from_path(2, PathBuf::from("b.png"), false),
            Item::from_path(3, PathBuf::from("c.pdf"), false),
        ];
        let groups = offline(&items);
        assert_eq!(groups[0].name, "Documents");
        assert_eq!(groups[0].item_ids, [1, 3]);
        validate(&groups, &items).unwrap();
        let mut bad = groups;
        bad[0].item_ids.push(2);
        assert!(validate(&bad, &items).is_err());
    }
    #[test]
    fn seventy_shortcuts_are_grouped_by_purpose_and_unknowns_remain_visible() {
        let apps = [
            "Firefox",
            "Visual Studio Code",
            "Blender",
            "Discord",
            "Steam",
            "VLC",
            "Audacity",
        ];
        let mut items = Vec::new();
        for (index, name) in apps.iter().cycle().take(70).enumerate() {
            items.push(Item::from_path(
                index as u64 + 1,
                format!("{name} - {}.lnk", index + 1).into(),
                false,
            ));
        }
        items.push(Item::from_path(71, "Unknown launcher.lnk".into(), false));
        let groups = offline(&items);
        validate(&groups, &items).unwrap();
        assert_eq!(groups.len(), 8);
        assert!(!groups.iter().any(|g| g.name == "Applications"));
        assert!(
            groups
                .iter()
                .filter(|g| g.name != "Needs review")
                .all(|g| g.item_ids.len() == 10)
        );
        assert_eq!(
            groups
                .iter()
                .find(|g| g.name == "Needs review")
                .unwrap()
                .item_ids,
            [71]
        );
    }
    #[test]
    fn selected_suggestions_preserve_unselected_references_and_reject_stale_metadata() {
        let root = std::env::current_dir().unwrap();
        let mut layout = Layout::default();
        layout.import(root.join("one.pdf"), false, 1);
        layout.import(root.join("two.png"), false, 1);
        let selected = vec![layout.items[0].clone()];
        let groups = offline(&selected);
        apply_selected(&mut layout, &groups, &selected).unwrap();
        layout.validate().unwrap();
        assert!(
            layout
                .groups
                .iter()
                .any(|g| g.name == "Inbox" && g.items.contains(&layout.items[1].id))
        );
        layout.items[0].name = "changed.pdf".into();
        assert!(apply_selected(&mut layout, &groups, &selected).is_err());
    }
    #[test]
    fn regrouping_preserves_the_category_region_and_roundtrips_its_screen_mode() {
        let mut layout = Layout::default();
        layout.groups[0].name = "Games".into();
        layout.groups[0].placement.region = Some(crate::regions::Region {
            left: 3,
            top: 0,
            columns: 3,
            rows: 2,
        });
        layout.import(std::env::current_dir().unwrap().join("Steam.lnk"), false, 1);
        let items = layout.items.clone();
        let groups = offline(&items);
        apply_selected(&mut layout, &groups, &items).unwrap();
        let restored: Layout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(
            restored.groups[0].placement.region,
            Some(crate::regions::Region {
                left: 3,
                top: 0,
                columns: 3,
                rows: 2
            })
        );
        assert!(restored.groups[0].on_screen);
    }
}
