use super::*;
pub(super) fn folder_subset(
    items: &[Item],
    groups: &[grouping::Suggestion],
    screen: &std::collections::BTreeSet<String>,
) -> (Vec<Item>, Vec<grouping::Suggestion>) {
    let folders: Vec<_> = groups
        .iter()
        .filter(|g| !screen.contains(&g.name))
        .cloned()
        .collect();
    let ids: HashSet<_> = folders
        .iter()
        .flat_map(|g| g.item_ids.iter().copied())
        .collect();
    (
        items
            .iter()
            .filter(|i| ids.contains(&i.id))
            .cloned()
            .collect(),
        folders,
    )
}
impl App {
    pub(super) unsafe fn choose_category_mode(&mut self) -> Result<()> {
        unsafe {
            let groups = ai::parse_content(&self.text(JSON), &self.scope)?;
            let row = SendMessageW(self.c(PROPOSALS), LB_GETCURSEL, 0, 0);
            ensure!(row >= 0, tr("Select a proposed group first"));
            let name = &groups[row as usize].name;
            if self.checked(SCREEN_CATEGORY) {
                self.layout.screen_categories.insert(name.clone());
            } else {
                self.layout.screen_categories.remove(name);
            }
            self.plan = None;
            self.pending_organization = None;
            self.set_suggestions(groups)?;
            self.changed();
            self.update_actions();
            self.status(
                "Category destination saved. Review organization to apply the combined plan.",
            );
            Ok(())
        }
    }
    pub(super) unsafe fn finish_organization(
        &mut self,
        groups: &[grouping::Suggestion],
    ) -> Result<()> {
        unsafe {
            let ids: HashSet<_> = groups
                .iter()
                .flat_map(|g| g.item_ids.iter().copied())
                .collect();
            let current: Vec<_> = self
                .layout
                .items
                .iter()
                .filter(|i| ids.contains(&i.id))
                .cloned()
                .collect();
            grouping::apply_selected(&mut self.layout, groups, &current)?;
            let names: HashSet<_> = groups.iter().map(|g| g.name.as_str()).collect();
            for group in &mut self.layout.groups {
                if names.contains(group.name.as_str()) {
                    group.on_screen = self.layout.screen_categories.contains(&group.name);
                }
            }
            self.scope = current;
            self.hidden = true;
            self.rebuild_groups()?;
            self.refresh();
            self.changed();
            if groups
                .iter()
                .any(|g| self.layout.screen_categories.contains(&g.name))
            {
                self.arrangement.show(self.hwnd, self.font, &self.layout);
            }
            self.status(
                "Organization applied. Arrange screen categories; file moves remain undoable.",
            );
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_plan_moves_only_folder_categories_and_keeps_screen_references_in_place() {
        let root = std::env::current_dir().unwrap();
        let items = vec![
            Item::from_path(1, root.join("Game.lnk"), false),
            Item::from_path(2, root.join("Notes.pdf"), false),
        ];
        let groups = vec![
            grouping::Suggestion {
                name: "Games".into(),
                item_ids: vec![1],
            },
            grouping::Suggestion {
                name: "Documents".into(),
                item_ids: vec![2],
            },
        ];
        let screen = std::collections::BTreeSet::from(["Games".into()]);
        let (files, folders) = folder_subset(&items, &groups, &screen);
        grouping::validate(&folders, &files).unwrap();
        assert_eq!(files.iter().map(|i| i.id).collect::<Vec<_>>(), [2]);
        assert_eq!(items[0].path, root.join("Game.lnk"));
    }
    #[test]
    fn mixed_filesystem_apply_and_undo_leave_screen_shortcuts_and_target_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(
            root.join("Game.url"),
            "[InternetShortcut]\nURL=steam://rungameid/123",
        )
        .unwrap();
        fs::write(root.join("Notes.txt"), "Synthetic notes").unwrap();
        let items = vec![
            Item::from_path(1, root.join("Game.url"), false),
            Item::from_path(2, root.join("Notes.txt"), false),
        ];
        let groups = vec![
            grouping::Suggestion {
                name: "Games".into(),
                item_ids: vec![1],
            },
            grouping::Suggestion {
                name: "Documents".into(),
                item_ids: vec![2],
            },
        ];
        let (files, folders) = folder_subset(
            &items,
            &groups,
            &std::collections::BTreeSet::from(["Games".into()]),
        );
        let mut plan = crate::operations::preview(root, &files, &folders).unwrap();
        let journal = root.join("journal.json");
        plan.apply(&journal).unwrap();
        assert!(root.join("Documents/Notes.txt").exists());
        assert!(root.join("Game.url").exists());
        assert!(!root.join("Games").exists());
        plan.undo(&journal).unwrap();
        assert!(root.join("Notes.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("Game.url")).unwrap(),
            "[InternetShortcut]\nURL=steam://rungameid/123"
        );
    }
}
