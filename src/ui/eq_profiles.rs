use std::collections::HashMap;

use crate::audio::effects::Settings;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct EqProfile {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) settings: Settings,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct EqProfileStore {
    pub(super) profiles: Vec<EqProfile>,
    output_links: HashMap<String, String>,
    next_id: u64,
}

impl EqProfileStore {
    pub(super) fn create(
        &mut self,
        name: &str,
        settings: Settings,
    ) -> Result<String, &'static str> {
        let name = clean_name(name)?;
        if !self.name_available(&name, None) {
            return Err("That profile name is already in use");
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("eq-{}-{created}-{}", std::process::id(), self.next_id);
        self.profiles.push(EqProfile {
            id: id.clone(),
            name,
            settings: settings.sanitized(),
        });
        Ok(id)
    }

    pub(super) fn get(&self, id: &str) -> Option<&EqProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub(super) fn update(&mut self, id: &str, settings: Settings) -> Result<(), &'static str> {
        let profile = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?;
        profile.settings = settings.sanitized();
        Ok(())
    }

    pub(super) fn rename(&mut self, id: &str, name: &str) -> Result<(), &'static str> {
        let name = clean_name(name)?;
        if self.get(id).is_none() {
            return Err("Profile not found");
        }
        if !self.name_available(&name, Some(id)) {
            return Err("That profile name is already in use");
        }
        self.profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?
            .name = name;
        Ok(())
    }

    fn name_available(&self, name: &str, except: Option<&str>) -> bool {
        !self
            .profiles
            .iter()
            .any(|p| Some(p.id.as_str()) != except && p.name.eq_ignore_ascii_case(name))
    }

    pub(super) fn delete(&mut self, id: &str) -> Result<(), &'static str> {
        let index = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or("Profile not found")?;
        self.profiles.remove(index);
        self.output_links.retain(|_, linked| linked != id);
        Ok(())
    }

    pub(super) fn link_output(&mut self, output: &str, id: &str) -> Result<(), &'static str> {
        if self.get(id).is_none() {
            return Err("Profile not found");
        }
        self.output_links.insert(output.to_owned(), id.to_owned());
        Ok(())
    }

    pub(super) fn unlink_output(&mut self, output: &str) {
        self.output_links.remove(output);
    }

    pub(super) fn for_output(&self, output: &str) -> Option<&String> {
        self.output_links
            .get(output)
            .filter(|id| self.get(id).is_some())
    }
}

pub(super) fn output_transition(
    store: &EqProfileStore,
    previous: Option<&str>,
    current: &str,
    baseline: Option<Settings>,
) -> Option<Settings> {
    if previous == Some(current) {
        return None;
    }
    store
        .for_output(current)
        .and_then(|id| store.get(id).map(|profile| profile.settings))
        .or(baseline)
}

fn selected_profile_id(store: &EqProfileStore, selected: Option<&str>) -> Option<String> {
    selected.filter(|id| store.get(id).is_some()).map(str::to_owned)
}

fn settings_for_selection(store: &EqProfileStore, selected: Option<&str>) -> Option<Settings> {
    selected.and_then(|id| store.get(id).map(|profile| profile.settings))
}

fn clean_name(name: &str) -> Result<String, &'static str> {
    let name = name.trim();
    if name.is_empty() {
        Err("Enter a profile name")
    } else if name.chars().count() > 40 {
        Err("Names can be up to 40 characters")
    } else {
        Ok(name.to_owned())
    }
}

#[derive(Clone)]
struct RenameDraft {
    profile_id: String,
    name: String,
}

pub(super) fn controls(
    ui: &mut egui::Ui,
    store: &mut EqProfileStore,
    settings: &mut Settings,
    baseline: &mut Option<Settings>,
    selected_profile: &mut Option<String>,
    output: Option<(&str, &str)>,
) {
    use egui::RichText;
    ui.separator();
    ui.label(RichText::new("Your profiles").strong());
    let selection_id = ui.make_persistent_id("eq-profile-selected");
    let rename_id = ui.make_persistent_id("eq-profile-rename-draft");
    let delete_id = ui.make_persistent_id("eq-profile-delete-target");
    *selected_profile = selected_profile_id(store, selected_profile.as_deref());
    let mut active = selected_profile.clone();
    let previous = active.clone();
    if store.profiles.is_empty() {
        ui.label("General EQ (used on outputs without a linked profile).");
    } else {
        let title = active.as_deref().and_then(|id| store.get(id)).map_or("Choose profile", |p| p.name.as_str());
        let picker = ui.button(format!("{title} ▾"));
        egui::Popup::menu(&picker).show(|ui| {
                for profile in &store.profiles {
                    if ui.button(&profile.name).clicked() {
                        active = Some(profile.id.clone());
                        *settings = profile.settings;
                        ui.close();
                    }
                }
            });
    }
    if active != previous {
        ui.data_mut(|d| {
            d.remove::<RenameDraft>(rename_id);
            d.remove::<String>(delete_id);
        });
    }
    *selected_profile = active.clone();
    ui.data_mut(|d| d.insert_temp(selection_id, active.clone()));

    let modified = active.as_deref().and_then(|id| store.get(id)).is_some_and(|p| p.settings != *settings);
    if let Some(id) = active.as_deref() {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Recall").on_hover_text("Revert to this saved profile").clicked() {
                if let Some(snapshot) = settings_for_selection(store, Some(id)) { *settings = snapshot; }
            }
            if ui.add_enabled(modified, egui::Button::new("Update profile")).on_hover_text("Replace this saved snapshot with the current sound controls").clicked() {
                let _ = store.update(id, *settings);
            }
            ui.label(if modified { "Modified" } else { "Matches saved profile" });
            if ui.button("Rename").clicked() {
                if let Some(profile) = store.get(id) {
                    ui.data_mut(|d| d.insert_temp(rename_id, RenameDraft { profile_id: id.to_owned(), name: profile.name.clone() }));
                }
            }
            if ui.button("Delete").clicked() { ui.data_mut(|d| d.insert_temp(delete_id, id.to_owned())); }
        });
    }

    let create_id = ui.make_persistent_id("eq-profile-create-name");
    let mut name: String = ui.data(|d| d.get_temp(create_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("New profile name").desired_width(170.0));
        let valid = !name.trim().is_empty() && name.chars().count() <= 40 && store.name_available(name.trim(), None);
        if ui.add_enabled(valid, egui::Button::new("Save as new")).clicked() {
            if let Ok(id) = store.create(&name, *settings) { *selected_profile = Some(id); name.clear(); }
        }
    });
    if !name.is_empty() && (name.chars().count() > 40 || !store.name_available(name.trim(), None)) {
        ui.label("Use a unique name of up to 40 characters.");
    }
    ui.data_mut(|d| d.insert_temp(create_id, name));

    if let Some(mut draft) = ui.data(|d| d.get_temp::<RenameDraft>(rename_id)) {
        if active.as_deref() != Some(&draft.profile_id) { ui.data_mut(|d| d.remove::<RenameDraft>(rename_id)); }
        else {
            let valid = !draft.name.trim().is_empty() && draft.name.chars().count() <= 40 && store.name_available(draft.name.trim(), Some(&draft.profile_id));
            let mut done = false;
            ui.horizontal(|ui| {
                ui.label(format!("Rename {}", store.get(&draft.profile_id).map_or("profile", |p| p.name.as_str())));
                ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(170.0));
                if ui.add_enabled(valid, egui::Button::new("Save name")).clicked() {
                    let _ = store.rename(&draft.profile_id, &draft.name);
                    done = true;
                }
                if ui.button("Cancel").clicked() { done = true; }
            });
            if !draft.name.trim().is_empty() && !valid { ui.label("Use a unique name of up to 40 characters."); }
            ui.data_mut(|d| {
                if done { d.remove::<RenameDraft>(rename_id); } else { d.insert_temp(rename_id, draft); }
            });
        }
    }

    if let Some(deleting) = ui.data(|d| d.get_temp::<String>(delete_id)) {
        if active.as_deref() != Some(&deleting) { ui.data_mut(|d| d.remove::<String>(delete_id)); }
        else if let Some(profile) = store.get(&deleting) {
            let profile_name = profile.name.clone();
            ui.horizontal(|ui| {
                ui.label(format!("Delete “{profile_name}” and remove its output links?"));
                if ui.button("Confirm delete").clicked() {
                    let was_linked_here = output.and_then(|(id, _)| store.for_output(id)).is_some_and(|id| id == &deleting);
                    let _ = store.delete(&deleting);
                    if was_linked_here && let Some(saved) = baseline.as_ref() { *settings = *saved; }
                    *selected_profile = None;
                    ui.data_mut(|d| d.remove::<String>(delete_id));
                }
                if ui.button("Cancel").clicked() { ui.data_mut(|d| d.remove::<String>(delete_id)); }
            });
        }
    }

    if let (Some((output_id, output_name)), Some(selected)) = (output, active.as_deref()) {
        let linked = store.for_output(output_id);
        let mut use_profile = linked.is_some_and(|id| id == selected);
        let label = format!("Use this profile on {output_name}");
        if ui.checkbox(&mut use_profile, label).changed() {
            if use_profile {
                if baseline.is_none() { *baseline = Some(*settings); }
                let _ = store.link_output(output_id, selected);
                if let Some(snapshot) = settings_for_selection(store, Some(selected)) { *settings = snapshot; }
            } else {
                store.unlink_output(output_id);
                if let Some(saved) = baseline.as_ref() { *settings = *saved; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::effects::Settings;

    #[test]
    fn saved_snapshot_keeps_all_eq_controls_and_is_independent_of_later_edits() {
        let mut store = EqProfileStore::default();
        let mut settings = Settings::default();
        settings.frequency_hz[15] = 17_500.0;
        settings.q[3] = 4.2;
        settings.bands[7] = -5.5;
        settings.enabled[2] = false;
        settings.preamp_db = -3.0;
        settings.bypass = true;
        settings.left = 71;
        settings.right = 83;
        let id = store.create("Night", settings).unwrap();
        let mut live = settings;
        live.bands[7] = 8.0;

        assert!(store.get(&id).unwrap().settings == settings_snapshot());
        assert!(store.get(&id).unwrap().settings != live);
    }

    fn settings_snapshot() -> Settings {
        let mut settings = Settings::default();
        settings.frequency_hz[15] = 17_500.0;
        settings.q[3] = 4.2;
        settings.bands[7] = -5.5;
        settings.enabled[2] = false;
        settings.preamp_db = -3.0;
        settings.bypass = true;
        settings.left = 71;
        settings.right = 83;
        settings
    }

    #[test]
    fn output_links_use_profile_identity_and_delete_clears_links() {
        let mut store = EqProfileStore::default();
        let first = store.create("Studio", Settings::default()).unwrap();
        let other = store.create("Travel", Settings::default()).unwrap();
        store.link_output("headphones-A", &first).unwrap();
        store.link_output("speakers-B", &other).unwrap();
        assert_eq!(store.for_output("headphones-A"), Some(&first));
        store.delete(&first).unwrap();
        assert_eq!(store.for_output("headphones-A"), None);
        assert_eq!(store.for_output("speakers-B"), Some(&other));
    }

    #[test]
    fn rename_and_explicit_update_do_not_change_other_profiles() {
        let mut store = EqProfileStore::default();
        let one = store.create("A", Settings::default()).unwrap();
        let two = store.create("B", Settings::default()).unwrap();
        let mut edited = Settings::default();
        edited.bands[0] = 6.0;
        store.update(&one, edited).unwrap();
        store.rename(&one, "Renamed").unwrap();
        assert_eq!(store.get(&one).unwrap().name, "Renamed");
        assert!(store.get(&one).unwrap().settings == edited);
        assert!(store.get(&two).unwrap().settings == Settings::default());
    }

    #[test]
    fn output_transitions_restore_baseline_on_unlinked_device_and_reapply_after_reconnect() {
        let mut store = EqProfileStore::default();
        let mut tuned = Settings::default();
        tuned.bands[0] = 7.0;
        let linked = store.create("Headphones", tuned).unwrap();
        store.link_output("A", &linked).unwrap();
        let mut baseline = Settings::default();
        baseline.left = 63;

        assert!(output_transition(&store, None, "A", Some(baseline)) == Some(tuned));
        assert!(output_transition(&store, Some("A"), "B", Some(baseline)) == Some(baseline));
        assert!(output_transition(&store, Some("A"), "A", Some(baseline)).is_none());
        assert!(output_transition(&store, None, "A", Some(baseline)) == Some(tuned));
    }

    #[test]
    fn unlinked_transition_keeps_legacy_settings_and_duplicate_names_are_rejected() {
        let store = EqProfileStore::default();
        let legacy = Settings::default();
        assert!(output_transition(&store, None, "legacy-output", Some(legacy)) == Some(legacy));

        let mut store = store;
        store.create("Desk", legacy).unwrap();
        assert!(store.create("desk", legacy).is_err());
    }

    #[test]
    fn profile_store_round_trips_for_signed_out_local_preferences() {
        let mut store = EqProfileStore::default();
        let id = store.create("Local", settings_snapshot()).unwrap();
        store.link_output("endpoint-id", &id).unwrap();
        let saved = serde_json::to_string(&store).unwrap();
        let restored: EqProfileStore = serde_json::from_str(&saved).unwrap();
        assert!(restored.get(&id).unwrap().settings == settings_snapshot());
        assert_eq!(restored.for_output("endpoint-id"), Some(&id));
    }

    #[test]
    fn selected_profile_identity_survives_reload_and_applies_that_snapshot() {
        let mut store = EqProfileStore::default();
        let mut settings = Settings::default();
        settings.bands[4] = 5.0;
        let selected = store.create("Saved", settings).unwrap();
        let saved = serde_json::to_string(&store).unwrap();
        let restored: EqProfileStore = serde_json::from_str(&saved).unwrap();

        let selected_after_reload = selected_profile_id(&restored, Some(&selected));
        assert_eq!(selected_after_reload.as_deref(), Some(selected.as_str()));
        assert!(settings_for_selection(&restored, selected_after_reload.as_deref()) == Some(settings));
        assert!(selected_profile_id(&restored, Some("deleted-id")).is_none());
    }
}
