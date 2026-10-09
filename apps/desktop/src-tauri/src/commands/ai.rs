//! IPC for AI assistance (`ai-assist`): provider settings, credentials, explain and suggest.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mergeiq_ai::context::{self, ChunkSpans, CommitInput, ContextInput, MAX_COMMITS_PER_SIDE};
use mergeiq_ai::privacy::RepoDecision;
use mergeiq_ai::secrets::{OsStore, Secret, SecretStore};
use mergeiq_ai::service::{build_provider, AiFailure, AiSettings, ProviderSettings};
use mergeiq_ai::usage::{SessionUsage, UsageRecord};
use mergeiq_ai::validate::{self, Checked};
use mergeiq_ai::{AiProvider, CancellationToken, Effort, ProviderKind, Task, TestReport, Usage};
use mergeiq_git::{Repo, RepoPath};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri_specta::Event;

use crate::repos::Repos;
use crate::settings;

/// Consent key for windows that are not part of a repository (command-line merges).
pub const STANDALONE_SCOPE: &str = "standalone";

/// Process-wide AI state: credential store, usage and running requests.
pub struct AiState {
    store: Arc<dyn SecretStore>,
    usage: Mutex<SessionUsage>,
    runs: Mutex<HashMap<String, CancellationToken>>,
    bodies: Mutex<HashMap<String, String>>,
}

impl AiState {
    /// State backed by the operating system's credential store.
    pub fn new() -> Self {
        Self::with_store(Arc::new(OsStore))
    }

    /// State backed by `store`.
    pub fn with_store(store: Arc<dyn SecretStore>) -> Self {
        Self {
            store,
            usage: Mutex::new(SessionUsage::default()),
            runs: Mutex::new(HashMap::new()),
            bodies: Mutex::new(HashMap::new()),
        }
    }

    fn register(&self, id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut runs) = self.runs.lock() {
            runs.insert(id.to_string(), token.clone());
        }
        token
    }

    fn unregister(&self, id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(id);
        }
    }

    fn key(&self, kind: ProviderKind) -> Option<Secret> {
        if !kind.needs_key() {
            return None;
        }
        match self.store.get(kind.id()) {
            Ok(k) => k,
            Err(err) => {
                tracing::warn!(provider = kind.id(), error = %err, "could not read the stored key");
                None
            }
        }
    }

    fn record(
        &self,
        settings: &AiSettings,
        kind: ProviderKind,
        model: &str,
        usage: Usage,
    ) -> (UsageRecord, UsageSummary) {
        let mut session = self.usage.lock().expect("usage lock");
        session.record(kind, model, usage, &settings.prices);
        let record = session.records.last().cloned().expect("just recorded");
        (record, UsageSummary::of(&session))
    }
}

impl Default for AiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Session totals without the per-request list.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub requests: u32,
    pub totals: Usage,
    pub cost: Option<f64>,
}

impl UsageSummary {
    fn of(s: &SessionUsage) -> Self {
        Self {
            requests: s.requests,
            totals: s.totals,
            cost: s.cost,
        }
    }
}

/// A streamed piece of an explanation (emitted as `ai-delta` to the requesting window).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct AiDelta {
    pub request_id: String,
    pub text: String,
}

/// A finished explanation.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiExplained {
    pub text: String,
    pub record: UsageRecord,
    pub session: UsageSummary,
    /// What the context builder left out.
    pub notes: Vec<String>,
}

/// A finished suggestion, checked but not applied.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiSuggested {
    pub checked: Checked,
    pub record: UsageRecord,
    pub session: UsageSummary,
    pub notes: Vec<String>,
    pub estimated_tokens: u32,
}

/// The exact text a request would send.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiPreview {
    pub text: String,
    pub estimated_tokens: u32,
    pub notes: Vec<String>,
    pub over_budget: bool,
    /// Host the request goes to.
    pub destination: String,
}

/// Pre-flight numbers for "suggest remaining".
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiEstimate {
    pub per_chunk: Vec<u32>,
    pub total_tokens: u32,
    pub over_budget: bool,
    pub provider: ProviderKind,
    pub model: String,
    /// Dollars for the input alone, when the model has a price.
    pub input_cost: Option<f64>,
}

/// What the editor needs to decide which AI controls to show.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    /// Why requests are not possible right now, if they are not.
    pub blocked: Option<AiFailure>,
    pub provider: ProviderKind,
    pub model: String,
    pub effort: Effort,
    /// Requests stay on this machine.
    pub local: bool,
    pub store_name: String,
}

/// Credential information; the key itself never leaves the credential store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub present: bool,
    pub last4: Option<String>,
    pub store_name: String,
}

/// Which action a preview is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AiTask {
    Explain,
    Suggest,
}

impl From<AiTask> for Task {
    fn from(t: AiTask) -> Self {
        match t {
            AiTask::Explain => Task::Explain,
            AiTask::Suggest => Task::Suggest,
        }
    }
}

fn internal(message: impl ToString) -> AiFailure {
    AiFailure::Internal {
        message: message.to_string(),
    }
}

/// The host part of a URL, for display.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    authority.to_string()
}

fn is_local(url: &str) -> bool {
    let host = host_of(url);
    let name = host
        .trim_start_matches('[')
        .split([':', ']'])
        .next()
        .unwrap_or("");
    matches!(name, "localhost" | "127.0.0.1" | "::1" | "") || name.ends_with(".localhost")
}

/// Everything a request needs, after the gates have passed.
pub struct Prepared {
    pub settings: AiSettings,
    pub kind: ProviderKind,
    pub model: String,
    pub provider: Arc<dyn AiProvider>,
    pub input: ContextInput,
}

/// `MERGEIQ_AI_MOCK=1` swaps in the offline mock provider, for demos and tests.
fn mock_requested() -> bool {
    std::env::var_os("MERGEIQ_AI_MOCK").is_some_and(|v| v != "0" && !v.is_empty())
}

fn effective(settings: &AiSettings) -> AiSettings {
    let mut s = settings.clone();
    if mock_requested() {
        s.provider = ProviderKind::Mock;
        s.confirmed = true;
        s.notice_accepted = true;
    }
    s
}

fn fill_commits(state: &AiState, repo: &Repo, input: &mut ContextInput) {
    let Ok(op) = repo.operation() else { return };
    let path = RepoPath::from_bytes(input.path.as_bytes().to_vec());
    let Ok(ctx) = repo.file_context(&path, &op) else {
        return;
    };
    let ours: Vec<_> = ctx.ours.into_iter().take(MAX_COMMITS_PER_SIDE).collect();
    let theirs: Vec<_> = ctx.theirs.into_iter().take(MAX_COMMITS_PER_SIDE).collect();

    let missing: Vec<String> = {
        let cache = state.bodies.lock().expect("bodies lock");
        ours.iter()
            .chain(&theirs)
            .filter(|c| !cache.contains_key(&c.sha))
            .map(|c| c.sha.clone())
            .collect()
    };
    if let Ok(found) = repo.commit_bodies(&missing) {
        let mut cache = state.bodies.lock().expect("bodies lock");
        for sha in &missing {
            // Remember commits without a body too, so they are not asked for again.
            cache.insert(sha.clone(), found.get(sha).cloned().unwrap_or_default());
        }
    }
    let cache = state.bodies.lock().expect("bodies lock");
    let convert = |commits: Vec<mergeiq_git::CommitSummary>| -> Vec<CommitInput> {
        commits
            .into_iter()
            .map(|c| CommitInput {
                body: cache.get(&c.sha).filter(|b| !b.is_empty()).cloned(),
                short_sha: c.short_sha,
                subject: c.subject,
            })
            .collect()
    };
    input.left.commits = convert(ours);
    input.right.commits = convert(theirs);
}

/// Runs the gates and builds the provider. Blocking (credential store and git).
pub fn prepare_blocking(
    state: &AiState,
    settings: &AiSettings,
    repo: Option<&Repo>,
    mut input: ContextInput,
) -> Result<Prepared, AiFailure> {
    let settings = effective(settings);
    let kind = settings.provider;
    let key = state.key(kind);
    let scope = repo.map_or_else(
        || STANDALONE_SCOPE.to_string(),
        |r| r.root().display().to_string(),
    );
    settings
        .gate(key.is_some(), &scope, &input.path)
        .map_err(AiFailure::from)?;
    if let Some(repo) = repo {
        fill_commits(state, repo, &mut input);
    }
    let ps = settings.for_provider(kind);
    let model = ps.model.clone();
    let provider = build_provider(kind, &ps, key);
    Ok(Prepared {
        settings,
        kind,
        model,
        provider,
        input,
    })
}

async fn prepare(
    state: &Arc<AiState>,
    repos: &Repos,
    repo: Option<u32>,
    input: ContextInput,
) -> Result<Prepared, AiFailure> {
    let repo = match repo {
        Some(id) => Some(
            repos
                .get(id)
                .ok_or_else(|| internal("no repository is open"))?,
        ),
        None => None,
    };
    let settings = settings::load().ai;
    let state = Arc::clone(state);
    tauri::async_runtime::spawn_blocking(move || {
        prepare_blocking(&state, &settings, repo.as_ref(), input)
    })
    .await
    .map_err(internal)?
}

/// Streams an explanation; `on_delta` receives each piece.
pub async fn explain_core(
    state: &AiState,
    prepared: Prepared,
    request_id: &str,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<AiExplained, AiFailure> {
    let token = state.register(request_id);
    let built = context::build(&prepared.input, Task::Explain, &prepared.settings.context);
    let result = prepared
        .provider
        .explain(&built.prompt, on_delta, &token)
        .await;
    state.unregister(request_id);
    let done = result.map_err(AiFailure::from)?;
    let (record, session) = state.record(
        &prepared.settings,
        prepared.kind,
        &prepared.model,
        done.usage,
    );
    Ok(AiExplained {
        text: done.value,
        record,
        session,
        notes: built.notes,
    })
}

/// Requests a suggestion and checks it. Nothing is applied.
pub async fn suggest_core(
    state: &AiState,
    prepared: Prepared,
    request_id: &str,
) -> Result<AiSuggested, AiFailure> {
    let token = state.register(request_id);
    let built = context::build(&prepared.input, Task::Suggest, &prepared.settings.context);
    let result = prepared.provider.suggest(&built.prompt, &token).await;
    state.unregister(request_id);
    let done = result.map_err(AiFailure::from)?;
    let input = prepared.input.clone();
    let suggestion = done.value;
    let checked = tauri::async_runtime::spawn_blocking(move || validate::check(&input, suggestion))
        .await
        .map_err(internal)?;
    let (record, session) = state.record(
        &prepared.settings,
        prepared.kind,
        &prepared.model,
        done.usage,
    );
    Ok(AiSuggested {
        checked,
        record,
        session,
        notes: built.notes,
        estimated_tokens: built.estimated_tokens,
    })
}

fn load_ai() -> AiSettings {
    settings::load().ai
}

fn save_ai(update: impl FnOnce(&mut AiSettings)) -> Result<AiSettings, AiFailure> {
    let mut all = settings::load();
    update(&mut all.ai);
    settings::save(&all).map_err(internal)?;
    Ok(all.ai)
}

#[tauri::command]
#[specta::specta]
pub fn ai_get_settings() -> AiSettings {
    load_ai()
}

#[tauri::command]
#[specta::specta]
pub fn ai_update_settings(settings: AiSettings) -> Result<AiSettings, AiFailure> {
    save_ai(|ai| *ai = settings)
}

/// Records (or clears, with `None`) the AI decision for a repository root.
#[tauri::command]
#[specta::specta]
pub fn ai_set_repo_decision(
    scope: String,
    decision: Option<RepoDecision>,
) -> Result<AiSettings, AiFailure> {
    save_ai(|ai| match decision {
        Some(d) => {
            ai.privacy.repos.insert(scope, d);
        }
        None => {
            ai.privacy.repos.remove(&scope);
        }
    })
}

async fn key_info(state: &Arc<AiState>, kind: ProviderKind) -> Result<KeyInfo, AiFailure> {
    let state = Arc::clone(state);
    tauri::async_runtime::spawn_blocking(move || {
        let key = state.store.get(kind.id()).map_err(internal)?;
        Ok(KeyInfo {
            present: key.is_some(),
            last4: key.as_ref().map(Secret::last4),
            store_name: state.store.name().to_string(),
        })
    })
    .await
    .map_err(internal)?
}

#[tauri::command]
#[specta::specta]
pub async fn ai_key_info(
    state: State<'_, Arc<AiState>>,
    provider: ProviderKind,
) -> Result<KeyInfo, AiFailure> {
    key_info(&state, provider).await
}

#[tauri::command]
#[specta::specta]
pub async fn ai_set_key(
    state: State<'_, Arc<AiState>>,
    provider: ProviderKind,
    key: String,
) -> Result<KeyInfo, AiFailure> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err(internal("The key is empty"));
    }
    let s = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        s.store.set(provider.id(), &key).map_err(internal)
    })
    .await
    .map_err(internal)??;
    key_info(&state, provider).await
}

#[tauri::command]
#[specta::specta]
pub async fn ai_delete_key(
    state: State<'_, Arc<AiState>>,
    provider: ProviderKind,
) -> Result<KeyInfo, AiFailure> {
    let s = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || s.store.delete(provider.id()).map_err(internal))
        .await
        .map_err(internal)??;
    key_info(&state, provider).await
}

/// "Test connection" with the form's current values; a typed `key` is used without being stored.
#[tauri::command]
#[specta::specta]
pub async fn ai_test_connection(
    state: State<'_, Arc<AiState>>,
    provider: ProviderKind,
    settings: ProviderSettings,
    key: Option<String>,
) -> Result<TestReport, AiFailure> {
    let s = Arc::clone(&state);
    let secret = tauri::async_runtime::spawn_blocking(move || {
        key.filter(|k| !k.trim().is_empty())
            .map(|k| Secret::new(k.trim()))
            .or_else(|| s.key(provider))
    })
    .await
    .map_err(internal)?;
    if provider.needs_key() && secret.is_none() {
        return Err(AiFailure::NotConfigured);
    }
    let p = build_provider(provider, &settings, secret);
    p.test().await.map_err(AiFailure::from)
}

#[tauri::command]
#[specta::specta]
pub fn ai_usage(state: State<'_, Arc<AiState>>) -> SessionUsage {
    state.usage.lock().expect("usage lock").clone()
}

#[tauri::command]
#[specta::specta]
pub fn ai_reset_usage(state: State<'_, Arc<AiState>>) {
    *state.usage.lock().expect("usage lock") = SessionUsage::default();
}

/// Which controls to offer for `path`, and why requests would be refused.
#[tauri::command]
#[specta::specta]
pub async fn ai_status(
    state: State<'_, Arc<AiState>>,
    repos: State<'_, Repos>,
    repo: Option<u32>,
    path: String,
) -> Result<AiStatus, AiFailure> {
    let scope = match repo {
        Some(id) => repos
            .get(id)
            .ok_or_else(|| internal("no repository is open"))?
            .root()
            .display()
            .to_string(),
        None => STANDALONE_SCOPE.to_string(),
    };
    let settings = effective(&load_ai());
    let s = Arc::clone(&state);
    let kind = settings.provider;
    let key = tauri::async_runtime::spawn_blocking(move || s.key(kind).is_some())
        .await
        .map_err(internal)?;
    let ps = settings.for_provider(kind);
    Ok(AiStatus {
        blocked: settings.gate(key, &scope, &path).err().map(AiFailure::from),
        provider: kind,
        local: kind == ProviderKind::Mock || is_local(&ps.base_url),
        model: ps.model,
        effort: ps.effort,
        store_name: state.store.name().to_string(),
    })
}

/// The exact text a request would send. Only the exclusion list can refuse a preview.
#[tauri::command]
#[specta::specta]
pub async fn ai_preview(
    repos: State<'_, Repos>,
    state: State<'_, Arc<AiState>>,
    repo: Option<u32>,
    input: ContextInput,
    task: AiTask,
) -> Result<AiPreview, AiFailure> {
    let settings = effective(&load_ai());
    settings
        .privacy
        .check_path(&input.path)
        .map_err(|_| AiFailure::Excluded)?;
    let repo = match repo {
        Some(id) => Some(
            repos
                .get(id)
                .ok_or_else(|| internal("no repository is open"))?,
        ),
        None => None,
    };
    let s = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let mut input = input;
        if let Some(repo) = &repo {
            fill_commits(&s, repo, &mut input);
        }
        let built = context::build(&input, task.into(), &settings.context);
        let ps = settings.active();
        AiPreview {
            text: built.preview(),
            estimated_tokens: built.estimated_tokens,
            notes: built.notes,
            over_budget: built.over_budget,
            destination: if settings.provider == ProviderKind::Mock {
                "this computer (mock provider)".to_string()
            } else {
                host_of(&ps.base_url)
            },
        }
    })
    .await
    .map_err(internal)
}

/// Estimated input tokens of suggesting every chunk in `chunks`.
#[tauri::command]
#[specta::specta]
pub async fn ai_estimate(
    repos: State<'_, Repos>,
    state: State<'_, Arc<AiState>>,
    repo: Option<u32>,
    input: ContextInput,
    chunks: Vec<ChunkSpans>,
) -> Result<AiEstimate, AiFailure> {
    let prepared = prepare(&state, &repos, repo, input).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let per_chunk: Vec<u32> = chunks
            .iter()
            .map(|c| {
                let mut input = prepared.input.clone();
                input.chunk = *c;
                context::build(&input, Task::Suggest, &prepared.settings.context).estimated_tokens
            })
            .collect();
        let total_tokens = per_chunk.iter().fold(0u32, |a, b| a.saturating_add(*b));
        let usage = Usage {
            input_tokens: total_tokens,
            ..Usage::default()
        };
        AiEstimate {
            over_budget: total_tokens
                > prepared
                    .settings
                    .context
                    .token_budget
                    .saturating_mul(u32::try_from(chunks.len().max(1)).unwrap_or(u32::MAX)),
            input_cost: prepared.settings.prices.estimate(&prepared.model, &usage),
            per_chunk,
            total_tokens,
            provider: prepared.kind,
            model: prepared.model,
        }
    })
    .await
    .map_err(internal)
}

/// Streams an explanation of the chunk to the calling window (`ai-delta` events).
#[tauri::command]
#[specta::specta]
pub async fn ai_explain(
    window: tauri::WebviewWindow,
    repos: State<'_, Repos>,
    state: State<'_, Arc<AiState>>,
    repo: Option<u32>,
    request_id: String,
    input: ContextInput,
) -> Result<AiExplained, AiFailure> {
    let prepared = prepare(&state, &repos, repo, input).await?;
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let id = request_id.clone();
    let mut on_delta = move |text: &str| {
        let _ = AiDelta {
            request_id: id.clone(),
            text: text.to_string(),
        }
        .emit_to(&app, label.as_str());
    };
    explain_core(&state, prepared, &request_id, &mut on_delta).await
}

/// Requests a checked suggestion for the chunk.
#[tauri::command]
#[specta::specta]
pub async fn ai_suggest(
    repos: State<'_, Repos>,
    state: State<'_, Arc<AiState>>,
    repo: Option<u32>,
    request_id: String,
    input: ContextInput,
) -> Result<AiSuggested, AiFailure> {
    let prepared = prepare(&state, &repos, repo, input).await?;
    suggest_core(&state, prepared, &request_id).await
}

/// Where to go when the home window opens: set by [`ai_open_settings`], read once on load.
#[derive(Default)]
pub struct PendingSettings(Mutex<Option<String>>);

/// Asks the home window to show a settings section (emitted as `open-settings`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct OpenSettings {
    pub section: String,
}

/// Brings up Settings in the home window (creating it if only repository windows are open).
#[tauri::command]
#[specta::specta]
pub fn ai_open_settings(
    app: tauri::AppHandle,
    pending: State<'_, PendingSettings>,
    section: String,
) -> Result<(), AiFailure> {
    *pending.0.lock().expect("pending lock") = Some(section.clone());
    if let Some(home) = app.get_webview_window("main") {
        let _ = home.unminimize();
        let _ = home.show();
        let _ = home.set_focus();
        let _ = OpenSettings { section }.emit_to(&app, "main");
        return Ok(());
    }
    tauri::WebviewWindowBuilder::from_config(&app, &app.config().app.windows[0])
        .and_then(tauri::WebviewWindowBuilder::build)
        .map_err(internal)?;
    Ok(())
}

/// The section requested while the home window did not exist yet, once.
#[tauri::command]
#[specta::specta]
pub fn take_pending_settings(pending: State<'_, PendingSettings>) -> Option<String> {
    pending.0.lock().expect("pending lock").take()
}

/// Cancels a running request. Unknown ids are ignored.
#[tauri::command]
#[specta::specta]
pub fn ai_cancel(state: State<'_, Arc<AiState>>, request_id: String) {
    let token = state
        .runs
        .lock()
        .ok()
        .and_then(|r| r.get(&request_id).cloned());
    if let Some(token) = token {
        token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mergeiq_ai::context::{SideInput, Span};
    use mergeiq_ai::mock::{MockProvider, Step};
    use mergeiq_ai::secrets::MemoryStore;
    use mergeiq_ai::{AiError, Confidence, Strategy, Suggestion};
    use std::time::Duration;

    fn input(path: &str) -> ContextInput {
        let base = "a\nb\nc\n";
        ContextInput {
            path: path.into(),
            base: base.into(),
            left: SideInput {
                label: "main".into(),
                text: "a\nLEFT\nc\n".into(),
                commits: vec![],
            },
            right: SideInput {
                label: "feature".into(),
                text: "a\nRIGHT\nc\n".into(),
                commits: vec![],
            },
            result: base.into(),
            chunk: ChunkSpans {
                base: Span { start: 1, end: 2 },
                left: Span { start: 1, end: 2 },
                right: Span { start: 1, end: 2 },
                result: Span { start: 1, end: 2 },
            },
        }
    }

    fn ready_settings() -> AiSettings {
        let mut s = AiSettings {
            confirmed: true,
            notice_accepted: true,
            ..AiSettings::default()
        };
        s.privacy
            .repos
            .insert(STANDALONE_SCOPE.into(), RepoDecision::Allowed);
        s
    }

    fn state_with_key() -> AiState {
        let store = MemoryStore::default();
        store.set("anthropic", "sk-ant-test-1234").unwrap();
        AiState::with_store(Arc::new(store))
    }

    fn prepared(steps: Vec<Step>, state: &AiState) -> Prepared {
        let settings = ready_settings();
        Prepared {
            settings,
            kind: ProviderKind::Mock,
            model: "mock-1".into(),
            provider: Arc::new(MockProvider::new(steps)),
            input: input("src/a.ts"),
        }
        .tap(|_| {
            let _ = state;
        })
    }

    trait Tap: Sized {
        fn tap(self, f: impl FnOnce(&Self)) -> Self {
            f(&self);
            self
        }
    }
    impl Tap for Prepared {}

    #[test]
    fn gates_map_to_failures_before_any_request() {
        let state = state_with_key();
        let run = |s: &AiSettings, path: &str| prepare_blocking(&state, s, None, input(path)).err();
        assert_eq!(
            run(&AiSettings::default(), "a.ts"),
            Some(AiFailure::NotConfigured)
        );
        let mut s = ready_settings();
        s.notice_accepted = false;
        assert_eq!(run(&s, "a.ts"), Some(AiFailure::NoticeRequired));
        let mut s = ready_settings();
        s.privacy.repos.clear();
        assert_eq!(run(&s, "a.ts"), Some(AiFailure::RepoUnasked));
        s.privacy
            .repos
            .insert(STANDALONE_SCOPE.into(), RepoDecision::Declined);
        assert_eq!(run(&s, "a.ts"), Some(AiFailure::RepoDeclined));
        let s = ready_settings();
        assert_eq!(run(&s, "config/.env.production"), Some(AiFailure::Excluded));
        assert_eq!(
            AiFailure::Excluded.to_string(),
            "File excluded from AI by your settings"
        );
        assert!(run(&s, "src/a.ts").is_none());
        // No key stored: not configured, even when everything else is set.
        let empty = AiState::with_store(Arc::new(MemoryStore::default()));
        assert_eq!(
            prepare_blocking(&empty, &s, None, input("a.ts")).err(),
            Some(AiFailure::NotConfigured)
        );
    }

    #[tokio::test]
    async fn explain_streams_and_records_usage() {
        let state = state_with_key();
        let p = prepared(vec![Step::Text("It was renamed.".into())], &state);
        let mut seen = String::new();
        let mut cb = |t: &str| seen.push_str(t);
        let done = explain_core(&state, p, "r1", &mut cb).await.unwrap();
        assert_eq!(done.text, "It was renamed.");
        assert_eq!(seen, "It was renamed.");
        assert_eq!(done.session.requests, 1);
        assert!(done.session.totals.output_tokens > 0);
        assert!(
            state.runs.lock().unwrap().is_empty(),
            "the run is forgotten"
        );
    }

    #[tokio::test]
    async fn session_totals_are_the_sum_of_three_suggestions() {
        let state = state_with_key();
        let mut last = None;
        let mut expected = Usage::default();
        for i in 0..3 {
            let p = prepared(vec![], &state);
            let done = suggest_core(&state, p, &format!("s{i}")).await.unwrap();
            expected.add(&done.record.usage);
            last = Some(done);
        }
        let last = last.unwrap();
        assert_eq!(last.session.requests, 3);
        assert_eq!(last.session.totals.input_tokens, expected.input_tokens);
        assert_eq!(last.session.totals.output_tokens, expected.output_tokens);
        assert_eq!(state.usage.lock().unwrap().records.len(), 3);
    }

    #[tokio::test]
    async fn suggest_returns_a_checked_suggestion_and_applies_nothing() {
        let state = state_with_key();
        let scripted = Suggestion {
            resolution: "LEFT\nRIGHT".into(),
            explanation: "Both.".into(),
            confidence: Confidence::High,
            strategy: Strategy::Both,
            risks: vec![],
        };
        let p = prepared(vec![Step::Suggestion(scripted)], &state);
        let done = suggest_core(&state, p, "r2").await.unwrap();
        // The missing trailing line break is restored for the document.
        assert_eq!(done.checked.suggestion.resolution, "LEFT\nRIGHT\n");
        assert_eq!(done.checked.syntax_warning, None);
        assert_eq!(done.record.model, "mock-1");
    }

    #[tokio::test]
    async fn failures_do_not_count_as_usage_and_cancel_is_reported() {
        let state = state_with_key();
        let p = prepared(vec![Step::Fail(AiError::Truncated)], &state);
        assert_eq!(
            suggest_core(&state, p, "r3").await.unwrap_err(),
            AiFailure::Truncated
        );
        assert_eq!(state.usage.lock().unwrap().requests, 0);

        let p = Prepared {
            provider: Arc::new(
                MockProvider::new(vec![Step::Text("a b c d e f g h i j".into())])
                    .with_delta_delay(Duration::from_millis(50)),
            ),
            ..prepared(vec![], &state)
        };
        let state = Arc::new(state);
        let s2 = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut cb = |_: &str| {};
            explain_core(&s2, p, "slow", &mut cb).await
        });
        for _ in 0..50 {
            if state.runs.lock().unwrap().contains_key("slow") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        state.runs.lock().unwrap().get("slow").unwrap().cancel();
        assert_eq!(task.await.unwrap().unwrap_err(), AiFailure::Cancelled);
    }

    #[test]
    fn hosts_and_locality() {
        assert_eq!(host_of("https://api.anthropic.com/v1"), "api.anthropic.com");
        assert_eq!(host_of("http://localhost:11434"), "localhost:11434");
        assert_eq!(
            host_of("https://user:pw@proxy.example:8443/x"),
            "proxy.example:8443"
        );
        assert!(is_local("http://localhost:11434"));
        assert!(is_local("http://127.0.0.1:11434/"));
        assert!(is_local("http://[::1]:11434"));
        assert!(!is_local("https://api.openai.com"));
        assert!(!is_local("http://192.168.1.5:11434"));
    }

    #[test]
    fn keys_are_never_part_of_settings_or_status_json() {
        let json = serde_json::to_string(&ready_settings()).unwrap();
        assert!(!json.contains("sk-"));
        let info = KeyInfo {
            present: true,
            last4: Some("1234".into()),
            store_name: "memory".into(),
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            serde_json::json!({"present": true, "last4": "1234", "storeName": "memory"})
        );
    }
}
