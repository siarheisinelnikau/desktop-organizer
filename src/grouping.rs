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
            .entry(item.category.clone())
            .or_default()
            .push(item.id);
    }
    groups
        .into_iter()
        .map(|(name, item_ids)| Suggestion { name, item_ids })
        .collect()
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
            revised
                .groups
                .iter_mut()
                .find(|g| g.id == id)
                .unwrap()
                .items = proposal.item_ids.clone();
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
}
