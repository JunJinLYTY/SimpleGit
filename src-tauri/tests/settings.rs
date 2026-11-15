use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use simplegit_lib::settings::{
    ModelProfileInput, SecretStore, SettingsStore, SystemSecretStore, DEFAULT_COMMIT_RULE,
};

#[derive(Default)]
struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemorySecretStore {
    fn set(&self, profile_id: &str, secret: &str) -> simplegit_lib::error::AppResult<()> {
        self.values
            .lock()
            .unwrap()
            .insert(profile_id.to_string(), secret.to_string());
        Ok(())
    }

    fn clear(&self, profile_id: &str) -> simplegit_lib::error::AppResult<()> {
        self.values.lock().unwrap().remove(profile_id);
        Ok(())
    }

    fn has(&self, profile_id: &str) -> simplegit_lib::error::AppResult<bool> {
        Ok(self.values.lock().unwrap().contains_key(profile_id))
    }

    fn get(&self, profile_id: &str) -> simplegit_lib::error::AppResult<Option<String>> {
        Ok(self.values.lock().unwrap().get(profile_id).cloned())
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("tokio runtime")
}

fn data_path() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    (directory, path)
}

fn input(
    id: Option<String>,
    name: &str,
    model: &str,
    api_key: Option<&str>,
    is_default: bool,
) -> ModelProfileInput {
    ModelProfileInput {
        id,
        name: name.to_string(),
        base_url: "https://example.test/v1".to_string(),
        model: model.to_string(),
        timeout_seconds: 60,
        is_default,
        api_key: api_key.map(ToString::to_string),
        clear_api_key: false,
    }
}

#[test]
fn default_settings_include_one_rule_and_no_models() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();

    let settings = runtime().block_on(store.get(&secrets)).unwrap();
    assert_eq!(settings.version, 1);
    assert_eq!(settings.models.len(), 0);
    assert_eq!(settings.active_model_id, None);
    assert!(settings.recent_repositories.is_empty());
    assert_eq!(settings.commit_rule, DEFAULT_COMMIT_RULE);
    assert!(settings.commit_rule.contains("Conventional Commits"));
}

#[test]
fn saves_multiple_model_profiles_in_order() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();

    runtime()
        .block_on(store.save_model(
            input(None, "First", "model-a", Some("key-a"), false),
            &secrets,
        ))
        .unwrap();
    let settings = runtime()
        .block_on(store.save_model(
            input(None, "Second", "model-b", Some("key-b"), false),
            &secrets,
        ))
        .unwrap();

    assert_eq!(
        settings
            .models
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>(),
        vec!["First", "Second"]
    );
    assert!(settings.models.iter().all(|profile| profile.has_api_key));
    assert_eq!(
        settings.active_model_id,
        Some(settings.models[0].id.clone())
    );
}

#[test]
fn editing_without_a_key_preserves_the_existing_secret() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();
    let created = runtime()
        .block_on(store.save_model(
            input(None, "First", "model-a", Some("old-key"), false),
            &secrets,
        ))
        .unwrap();
    let id = created.models[0].id.clone();

    let updated = runtime()
        .block_on(store.save_model(
            input(Some(id.clone()), "Renamed", "model-b", None, false),
            &secrets,
        ))
        .unwrap();

    assert_eq!(updated.models[0].name, "Renamed");
    assert!(updated.models[0].has_api_key);
    assert_eq!(secrets.get(&id).unwrap().as_deref(), Some("old-key"));
}

#[test]
fn a_new_key_replaces_the_stored_secret() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();
    let created = runtime()
        .block_on(store.save_model(
            input(None, "First", "model-a", Some("old-key"), false),
            &secrets,
        ))
        .unwrap();
    let id = created.models[0].id.clone();

    runtime()
        .block_on(store.save_model(
            input(Some(id.clone()), "First", "model-a", Some("new-key"), false),
            &secrets,
        ))
        .unwrap();

    assert_eq!(secrets.get(&id).unwrap().as_deref(), Some("new-key"));
}

#[test]
fn deleting_the_active_profile_removes_its_secret_and_selects_the_next_model() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();
    let first = runtime()
        .block_on(store.save_model(
            input(None, "First", "model-a", Some("key-a"), false),
            &secrets,
        ))
        .unwrap();
    let first_id = first.models[0].id.clone();
    runtime()
        .block_on(store.save_model(
            input(None, "Second", "model-b", Some("key-b"), false),
            &secrets,
        ))
        .unwrap();

    let settings = runtime()
        .block_on(store.delete_model(&first_id, &secrets))
        .unwrap();

    assert_eq!(settings.models.len(), 1);
    assert_eq!(settings.models[0].name, "Second");
    assert_eq!(
        settings.active_model_id,
        Some(settings.models[0].id.clone())
    );
    assert!(!secrets.has(&first_id).unwrap());
}

#[test]
fn setting_default_clears_the_previous_default() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();
    let secrets = MemorySecretStore::default();
    let first = runtime()
        .block_on(store.save_model(input(None, "First", "model-a", None, true), &secrets))
        .unwrap();
    let first_id = first.models[0].id.clone();
    let second = runtime()
        .block_on(store.save_model(input(None, "Second", "model-b", None, true), &secrets))
        .unwrap();

    assert_eq!(
        second
            .models
            .iter()
            .filter(|profile| profile.is_default)
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        vec![second.models[1].id.as_str()]
    );
    assert!(
        !second
            .models
            .iter()
            .find(|profile| profile.id == first_id)
            .unwrap()
            .is_default
    );
}

#[test]
fn saves_and_reloads_all_non_secret_fields() {
    let (_directory, path) = data_path();
    let secrets = MemorySecretStore::default();
    let _saved = runtime().block_on(async {
        let store = SettingsStore::load(path.clone()).await.unwrap();
        store
            .save_model(input(None, "Local", "qwen", Some("secret"), true), &secrets)
            .await
            .unwrap();
        store
            .save_commit_rule("fix(git): 使用自定义规则")
            .await
            .unwrap();
        store
            .record_recent_repository(PathBuf::from("D:/Projects/One"))
            .await
            .unwrap()
    });

    let reloaded = runtime().block_on(async {
        let store = SettingsStore::load(path).await.unwrap();
        store.get(&secrets).await.unwrap()
    });

    assert_eq!(reloaded.commit_rule, "fix(git): 使用自定义规则");
    assert_eq!(reloaded.recent_repositories, vec!["D:/Projects/One"]);
    assert_eq!(reloaded.models[0].model, "qwen");
    assert_eq!(reloaded.models[0].timeout_seconds, 60);
    assert!(reloaded.models[0].has_api_key);
}

#[test]
fn corrupted_settings_return_an_error_and_are_not_overwritten() {
    let (_directory, path) = data_path();
    fs::write(&path, "{ definitely-not-json").unwrap();

    let error = runtime()
        .block_on(SettingsStore::load(path.clone()))
        .expect_err("corrupted settings must fail");

    assert_eq!(error.code, "SETTINGS_FAILED");
    assert_eq!(fs::read_to_string(path).unwrap(), "{ definitely-not-json");
}

#[test]
fn recent_repositories_are_deduplicated_and_limited() {
    let (_directory, path) = data_path();
    let store = runtime().block_on(SettingsStore::load(path)).unwrap();

    let settings = runtime().block_on(async {
        for index in 0..11 {
            store
                .record_recent_repository(PathBuf::from(format!("D:/Projects/{index}")))
                .await
                .unwrap();
        }
        store
            .record_recent_repository(PathBuf::from("D:/Projects/5"))
            .await
            .unwrap()
    });

    assert_eq!(settings.recent_repositories.len(), 10);
    assert_eq!(settings.recent_repositories[0], "D:/Projects/5");
    assert_eq!(
        settings
            .recent_repositories
            .iter()
            .filter(|path| path.as_str() == "D:/Projects/5")
            .count(),
        1
    );
}

#[cfg(target_os = "windows")]
#[test]
fn system_secret_store_round_trips_a_test_credential() {
    let store = SystemSecretStore::new("com.simplegit.desktop.tests");
    let profile_id = uuid::Uuid::new_v4().to_string();
    store.set(&profile_id, "round-trip-secret").unwrap();
    let observed = (store.has(&profile_id), store.get(&profile_id));
    store.clear(&profile_id).unwrap();

    assert!(observed.0.unwrap());
    assert_eq!(observed.1.unwrap().as_deref(), Some("round-trip-secret"));
    assert!(!store.has(&profile_id).unwrap());
}
