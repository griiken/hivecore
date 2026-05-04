//! Skeleton ACP server. Iterates a session pool keyed by `SessionId`. For
//! each `prompt` request, looks up (or lazily builds) an `AgentLoop` and
//! drives it to completion, streaming `session/update` notifications via
//! the bridge.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use std::sync::Arc as StdArc;

use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry};
use hivecore_builtin_tools::{default_set, WorkspaceRoot};
use hivecore_config::{Agent, AgentLoader, AgentRegistry};
use hivecore_execution_env::LocalEnv;
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_runtime_core::{AbortSignal, ExecutionEnv, ModelAdapter, Tool, ToolHook};
use hivecore_skills::{render::RenderCtx, SkillLoader, SkillRegistry, SubAgentRouter};
use tokio::sync::Mutex;

/// Per-process configuration. Built from env + cli at boot.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub api_key: String,
    pub workspace: PathBuf,
    /// Directory holding user-supplied `*.toml` agent files. Builtin
    /// agents (shipped with hivecore) are layered in beneath.
    pub agent_dir: Option<PathBuf>,
    /// Agent to use unless the client requests another.
    pub default_agent: String,
    /// Optional dir holding `*.md` skill files. When set, skills are loaded
    /// at boot and exposed to the agent alongside builtin tools.
    pub skill_dir: Option<PathBuf>,
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?;
        let workspace = std::env::var("HIVECORE_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().expect("cwd"));
        let agent_dir = std::env::var("HIVECORE_AGENT_DIR").ok().map(PathBuf::from);
        let default_agent = std::env::var("HIVECORE_AGENT").unwrap_or_else(|_| "default".into());
        let skill_dir = std::env::var("HIVECORE_SKILL_DIR").ok().map(PathBuf::from);
        Ok(Self {
            api_key,
            workspace,
            agent_dir,
            default_agent,
            skill_dir,
        })
    }
}

/// Shared state across all sessions in this process.
pub struct ServerState {
    pub config: ServerConfig,
    pub sessions: Mutex<HashMap<String, Arc<Mutex<AgentLoop>>>>,
    pub model: Arc<dyn ModelAdapter>,
    pub agents: AgentRegistry,
    pub skills: SkillRegistry,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("config", &self.config)
            .field("agents", &self.agents.ids())
            .finish()
    }
}

impl ServerState {
    pub fn new(config: ServerConfig) -> anyhow::Result<Arc<Self>> {
        let client = OpenAiClient::new(OpenAiConfig::new(config.api_key.clone()))?;
        let adapter: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));
        let agents = load_agent_registry(&config)?;
        if agents.is_empty() {
            anyhow::bail!("no agents registered — provide HIVECORE_AGENT_DIR or ship a builtin");
        }
        let skills = if let Some(dir) = &config.skill_dir {
            let loaded = SkillLoader::new().add_dir(dir).load()?;
            tracing::info!(count = loaded.len(), dir = %dir.display(), "skills loaded");
            SkillRegistry::from_skills(loaded)?
        } else {
            SkillRegistry::new()
        };
        Ok(Arc::new(Self {
            config,
            sessions: Mutex::new(HashMap::new()),
            model: adapter,
            agents,
            skills,
        }))
    }

    /// Build (or fetch) the `AgentLoop` for a session id, attaching `sink`
    /// and any `tool_hooks` (e.g. `ApprovalHook` for HITL — ADR-029).
    /// Hooks attach only on first construction; subsequent calls returning
    /// the cached `AgentLoop` ignore them.
    pub async fn ensure_session(
        &self,
        session_id: &str,
        sink: Arc<dyn hivecore_runtime_core::EventSink>,
        tool_hooks: Vec<Arc<dyn ToolHook>>,
    ) -> anyhow::Result<Arc<Mutex<AgentLoop>>> {
        let mut map = self.sessions.lock().await;
        if let Some(existing) = map.get(session_id) {
            return Ok(existing.clone());
        }
        let agent = self
            .agents
            .get(&self.config.default_agent)
            .or_else(|| self.agents.first())
            .ok_or_else(|| anyhow::anyhow!("no agent registered"))?;

        let root = WorkspaceRoot::new(&self.config.workspace)
            .map_err(|e| anyhow::anyhow!("invalid workspace: {e}"))?;
        let env: Arc<dyn ExecutionEnv> = Arc::new(LocalEnv::new(root.path().to_path_buf()));
        let all_tools = default_set(root, env);
        let mut filtered = filter_tools(&all_tools, agent);

        // Append skill-derived tools. Skills bypass the agent's tool
        // allowlist — opting them in by placing a `*.md` in
        // `HIVECORE_SKILL_DIR` is the explicit grant.
        let workdir = self.config.workspace.clone();
        let session_id_str = session_id.to_string();
        let router: Arc<dyn SubAgentRouter> = Arc::new(AgentRouter {
            agents: self.agents.clone(),
            model: self.model.clone(),
            workspace: self.config.workspace.clone(),
        });
        let skill_tools = self.skills.model_invokable_tools_with_router(
            move || RenderCtx {
                workdir: workdir.clone(),
                session_id: session_id_str.clone(),
                ..Default::default()
            },
            Some(router),
        );
        for st in skill_tools {
            filtered.push(st);
        }
        let tools = ToolRegistry::new(filtered);

        tracing::info!(
            agent = %agent.id,
            model = %agent.model.id,
            tools = ?tools.descriptors().iter().map(|d| d.name.clone()).collect::<Vec<_>>(),
            "session started"
        );

        let mut builder = AgentLoop::builder()
            .system_prompt(agent.system_prompt.clone())
            .model(self.model.clone(), agent.model.id.clone())
            .tools(tools)
            .sink(sink)
            .max_iterations(agent.limits.max_iterations as usize);
        for h in tool_hooks {
            builder = builder.hook(h);
        }
        let agent = builder.build()?;
        let arc = Arc::new(Mutex::new(agent));
        map.insert(session_id.to_string(), arc.clone());
        Ok(arc)
    }
}

/// Routes `agent: <agent-id>` skill delegations into a fresh `AgentLoop`
/// instantiated from the named agent. Mirrors the Claude Code pattern
/// where a skill says `agent: Explore` and the runtime forks into the
/// `Explore` sub-agent.
#[derive(Clone)]
struct AgentRouter {
    agents: AgentRegistry,
    model: Arc<dyn ModelAdapter>,
    workspace: PathBuf,
}

impl std::fmt::Debug for AgentRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRouter").finish()
    }
}

#[async_trait::async_trait]
impl SubAgentRouter for AgentRouter {
    async fn run(
        &self,
        agent_name: &str,
        prompt: String,
        signal: hivecore_runtime_core::AbortSignal,
    ) -> hivecore_runtime_core::RuntimeResult<String> {
        use hivecore_runtime_core::{AgentMessage, ContentBlock, RuntimeError};

        let agent = self.agents.get(agent_name).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("unknown sub-agent agent: {agent_name}"))
        })?;
        let root = WorkspaceRoot::new(&self.workspace)
            .map_err(|e| RuntimeError::ToolFailed(format!("workspace: {e}")))?;
        let env: Arc<dyn ExecutionEnv> = Arc::new(LocalEnv::new(root.path().to_path_buf()));
        let all_tools = default_set(root, env);
        let filtered = filter_tools(&all_tools, agent);
        let tools = ToolRegistry::new(filtered);

        let mut child = AgentLoop::builder()
            .system_prompt(agent.system_prompt.clone())
            .model(self.model.clone(), agent.model.id.clone())
            .tools(tools)
            .max_iterations(agent.limits.max_iterations as usize)
            .build()
            .map_err(|e| RuntimeError::ToolFailed(e.to_string()))?;
        let _ = child
            .run(user_text(prompt), signal)
            .await
            .map_err(|e| RuntimeError::ToolFailed(e.to_string()))?;
        let final_text = child
            .state()
            .messages
            .iter()
            .rev()
            .find_map(|m| match m {
                AgentMessage::Assistant { content, .. } => {
                    let t = content
                        .iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                }
                _ => None,
            })
            .unwrap_or_else(|| "(sub-agent produced no text)".into());
        Ok(final_text)
    }
}

fn filter_tools(all: &[StdArc<dyn Tool>], agent: &Agent) -> Vec<StdArc<dyn Tool>> {
    all.iter()
        .filter(|t| agent.tools.allows(t.name()))
        .cloned()
        .collect()
}

fn load_agent_registry(config: &ServerConfig) -> anyhow::Result<AgentRegistry> {
    let loader = AgentLoader::new();
    let mut all = Vec::new();

    // Builtin agents shipped with the spike. Path is relative to this
    // crate's manifest dir at compile time so the binary is self-contained.
    let builtin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("config/agents/builtin"))
        .unwrap_or_else(|| PathBuf::from("../config/agents/builtin"));
    if builtin_dir.exists() {
        all.extend(loader.load_dir(&builtin_dir)?);
    }
    // User overrides — appended last, but registry insertion rejects
    // duplicates so user overrides must use a different `id`.
    if let Some(dir) = &config.agent_dir {
        all.extend(loader.load_dir(dir)?);
    }
    Ok(AgentRegistry::from_agents(all)?)
}

/// Run a single in-process prompt turn against `agent`. The supplied
/// `AbortSignal` lets us honour `session/cancel` notifications.
pub async fn run_prompt_turn(
    agent: Arc<Mutex<AgentLoop>>,
    prompt_text: String,
    signal: AbortSignal,
) -> anyhow::Result<hivecore_runtime_core::StopReason> {
    let mut a = agent.lock().await;
    let outcome = a.run(user_text(prompt_text), signal).await?;
    Ok(outcome.stop_reason)
}

// Re-export the bridge type so the bin-side adapter can construct sinks
// without reaching into the bridge module.
pub use crate::bridge::acp_stop_reason;
