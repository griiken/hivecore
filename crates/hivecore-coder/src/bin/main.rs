//! `hivecore-coder` — sample CLI coding-agent harness.
//!
//! Usage:
//!   hivecore-coder "your prompt"
//!   hivecore-coder --continue "follow-up"          # latest by cwd
//!   hivecore-coder --session <id|name> "..."       # explicit
//!   hivecore-coder --name auth-refactor "..."      # set human-readable name
//!
//! Sessions persist as JSONL under `<HIVECORE_SESSIONS_DIR>/<tenant>/<id>.jsonl`
//! (default `~/.hivecore/sessions/default/<id>.jsonl`). ADR-026.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry};
use hivecore_browser_core::{BrowserProvider, TenantId as BrowserTenantId};
use hivecore_browser_harness::{default_tools as browser_default_tools, BrowserHarness};
use hivecore_browser_runtime::{ChromeConfig, ChromeProvider};
use hivecore_builtin_tools::{default_set, WorkspaceRoot};
use hivecore_coder::{CliPrompter, ClockTool, StderrSink, TurnBudgetHook};
use hivecore_compaction::{CompactionConfig, SummarizingTransform, DEFAULT_COMPACTION_CONFIG};
use hivecore_config::{Role, RoleLoader};
use hivecore_execution_env::LocalEnv;
use hivecore_mcp_client::{default_meta_tools, McpClient, McpRiskAugmenter, McpUserConfig};
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_persistence::{
    acquire_lock, append_entry, find_latest_by_cwd, resolve_handle, session_path, tenant_dir,
    AuditWriter, SessionHeader, SessionIndexEntry, SessionReader, SessionWriter, TenantId,
};
use hivecore_runtime_core::{
    AbortSignal, AgentMessage, Approval, ContentBlock, ContextTransform, EventSink, ExecutionEnv,
    FanOutSink, LifecycleHook, ModelAdapter, RiskAugmenter, SessionId, Tool,
};
use hivecore_tool_policy::{apply_exclude, ApprovalHook, ApprovalPolicy, ToolNameMatcher};

const SYSTEM_PROMPT: &str =
    "You are a terse coding assistant operating in a local workspace. You can read files, \
     edit files, run bash, and grep. You also have a `clock` tool that returns UTC time. \
     When `browser_*` tools are available you can drive a real Chrome browser: \
     open a session via `browser_session{op:open,name:...}`, navigate, snapshot to get \
     versioned element refs, then act on those refs. Always re-snapshot after navigation. \
     When `mcp_*` tools are available you can talk to MCP servers: call `mcp_servers` first \
     to see what's configured, `mcp_discover{server}` for tool schemas, then `mcp_call{server,tool,arguments}`. \
     Use tools when useful; respond with a one-line summary when done.";

#[derive(Parser, Debug)]
#[command(version, about = "Hivecore CLI coding agent")]
struct Args {
    /// Resume the most recent session in this cwd.
    #[arg(long, conflicts_with = "session")]
    r#continue: bool,

    /// Resume a specific session by id (UUID) or human-readable name.
    #[arg(long)]
    session: Option<String>,

    /// Assign a human-readable name to the session (persisted in the index).
    #[arg(long)]
    name: Option<String>,

    /// Disable compaction (debug / test path).
    #[arg(long)]
    no_compact: bool,

    /// Override the model context window in tokens. Default: 128_000.
    #[arg(long)]
    context_window: Option<u32>,

    /// Override `recent_turns_to_keep` (debug / small-window testing).
    #[arg(long)]
    recent_turns_to_keep: Option<usize>,

    /// Disable browser tools (skip spawning Chrome).
    #[arg(long)]
    no_browser: bool,

    /// Disable MCP integration (skip reading mcp.toml + registering meta-tools).
    #[arg(long)]
    no_mcp: bool,

    /// Disable HITL approval prompts (auto-allow risky tools — for autonomous runs).
    #[arg(long)]
    no_approval: bool,

    /// Drop tools from the model's tool list entirely (defense-in-depth — model
    /// can't try what it can't see). Comma-separated or repeat the flag.
    /// Example: `--exclude bash,write_file` for read-only investigation mode.
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,

    /// Apply a role overlay (ADR-033) to the agent's base system prompt for
    /// this session. Looks up `<role-dir>/<name>.toml` (default `<workspace>/
    /// .hivecore/roles/`). Role's `tools` field, if present, replaces the
    /// tool registry at this scope (defence-in-depth allowlist).
    #[arg(long)]
    role: Option<String>,

    /// Override the directory roles are loaded from. Defaults to
    /// `<workspace>/.hivecore/roles/`. Tilde and relative paths resolve
    /// against the current working directory.
    #[arg(long)]
    role_dir: Option<PathBuf>,

    /// The prompt to send. Multiple words are joined with spaces.
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args = Args::parse();
    let prompt = args.prompt.join(" ").trim().to_string();
    if prompt.is_empty() {
        anyhow::bail!(
            "usage: hivecore-coder [--continue|--session <id|name>] [--name <handle>] \"<prompt>\""
        );
    }

    let api_key =
        std::env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?;
    let model_id = std::env::var("HIVECORE_MODEL").unwrap_or_else(|_| "gpt-5.4-nano".to_string());
    let max_calls: usize = std::env::var("HIVECORE_TURN_BUDGET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let workspace = std::env::var("HIVECORE_WORKSPACE")
        .map_or_else(|_| std::env::current_dir().unwrap(), PathBuf::from);
    let tenant = std::env::var("HIVECORE_TENANT").unwrap_or_else(|_| "default".to_string());
    let sessions_dir = std::env::var("HIVECORE_SESSIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".hivecore").join("sessions")
        });

    // ---- resolve / create session id + replay prior messages -------------
    let tenant_root = tenant_dir(&sessions_dir, &tenant);
    let (session_id, prior_messages, header_existed) =
        resolve_session(&tenant_root, &workspace, &args).await?;

    let path = session_path(&sessions_dir, &tenant, session_id);
    let _lock = acquire_lock(&path)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let writer = if header_existed {
        SessionWriter::open_append(&path).await?
    } else {
        SessionWriter::create(
            &path,
            SessionHeader::new(session_id, &model_id, SYSTEM_PROMPT),
        )
        .await?
    };

    // Touch the index — record this session's name/cwd as of now.
    append_entry(
        &tenant_root,
        &SessionIndexEntry {
            session_id,
            cwd: workspace.clone(),
            name: args.name.clone(),
            updated_at: chrono::Utc::now(),
        },
    )
    .await?;

    // ---- substrate wiring ------------------------------------------------
    let mut openai_cfg = OpenAiConfig::new(api_key);
    if let Ok(base) = std::env::var("HIVECORE_OPENAI_BASE_URL") {
        openai_cfg = openai_cfg.with_base_url(base);
    }
    let client = OpenAiClient::new(openai_cfg)?;
    let model: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));

    let root = WorkspaceRoot::new(&workspace)?;
    let env: Arc<dyn ExecutionEnv> = Arc::new(LocalEnv::new(root.path().to_path_buf()));
    let mut tools: Vec<Arc<dyn Tool>> = default_set(root, env);
    tools.push(Arc::new(ClockTool));

    // ADR-027 — opt-in MCP integration. Reads `~/.hivecore/mcp.toml` (user)
    // then layers `<workspace>/.hivecore/mcp.toml` on top. Registers the three
    // lazy meta-tools (`mcp_servers` / `mcp_discover` / `mcp_call`) when any
    // server is configured. `--no-mcp` forces off.
    let mut mcp_risk_augmenter: Option<Arc<dyn RiskAugmenter>> = None;
    if !args.no_mcp {
        let mut mcp_cfg = McpUserConfig::default();
        let home = std::env::var("HOME").ok().map(PathBuf::from);
        if let Some(h) = &home {
            let p = h.join(".hivecore").join("mcp.toml");
            match McpUserConfig::from_path(&p) {
                Ok(c) => mcp_cfg.merge(c),
                Err(e) => eprintln!("[mcp ] skip {}: {e}", p.display()),
            }
        }
        let ws_path = workspace.join(".hivecore").join("mcp.toml");
        match McpUserConfig::from_path(&ws_path) {
            Ok(c) => mcp_cfg.merge(c),
            Err(e) => eprintln!("[mcp ] skip {}: {e}", ws_path.display()),
        }
        if mcp_cfg.has_any() {
            let n_enabled = mcp_cfg.enabled_count();
            let client = McpClient::new(mcp_cfg).with_tenant(tenant.clone());
            // ADR-029 A3 — `McpRiskAugmenter` reads cached
            // `ToolAnnotations.read_only_hint` so `policy=UnlessTrusted`
            // can short-circuit the human prompt for confirmed-readonly
            // MCP tools (e.g. `git_status`).
            mcp_risk_augmenter = Some(McpRiskAugmenter::into_arc(client.clone()));
            for t in default_meta_tools(client) {
                tools.push(t);
            }
            eprintln!(
                "[mcp ] {n_enabled} server(s) enabled; meta-tools registered (mcp_servers / mcp_discover / mcp_call)"
            );
        }
    }

    // ADR-028 — opt-in browser harness. `--no-browser` skips spawning Chrome.
    let _browser_harness_keep_alive = if args.no_browser {
        None
    } else {
        let provider: Arc<dyn BrowserProvider> =
            Arc::new(ChromeProvider::new(ChromeConfig::default()));
        let harness = BrowserHarness::new(provider, BrowserTenantId::new(tenant.clone()));
        for t in browser_default_tools(harness.clone()) {
            tools.push(t);
        }
        Some(harness)
    };

    // ADR-033 — role overlay. v0.1 binary surface supports session scope
    // only (one `--role <name>` per process; the call/session/agent axis
    // lands when `/role` slash commands and `SpawnAgentTool` argument
    // wiring follow). Role is a runtime overlay; the on-disk session
    // header keeps the base `SYSTEM_PROMPT` for replay parity.
    let role: Option<Role> = if let Some(name) = args.role.as_deref() {
        let dir = args
            .role_dir
            .clone()
            .unwrap_or_else(|| workspace.join(".hivecore").join("roles"));
        let role_path = dir.join(format!("{name}.toml"));
        let r = RoleLoader::new()
            .load_file(&role_path)
            .map_err(|e| anyhow::anyhow!("role `{name}` load failed: {e}"))?;
        eprintln!(
            "[role  ] applying `{}` from {}",
            r.name,
            role_path.display()
        );
        Some(r)
    } else {
        None
    };

    // Apply role.tools allowlist override BEFORE --exclude refinement.
    // Allowlist shrinks the tool set to exactly the listed names; exclude
    // can then drop further. Defence-in-depth: model never sees a tool the
    // role didn't sanction.
    let tools = if let Some(allowlist) = role.as_ref().and_then(|r| r.tools.as_ref()) {
        let allow: std::collections::HashSet<&str> = allowlist.iter().map(String::as_str).collect();
        let n_before = tools.len();
        let filtered: Vec<Arc<dyn Tool>> = tools
            .into_iter()
            .filter(|t| allow.contains(t.name()))
            .collect();
        eprintln!(
            "[role  ] tools allowlist kept {} of {n_before}: {:?}",
            filtered.len(),
            allowlist
        );
        filtered
    } else {
        tools
    };

    // ADR-029 A2 — static tool exclusion (Continue.dev `exclude` shape).
    // Filter applies after every tool source has registered (builtins +
    // MCP meta + browser) so the user can drop any of them.
    let tools = if args.exclude.is_empty() {
        tools
    } else {
        let n_before = tools.len();
        let filtered = apply_exclude(tools, args.exclude.clone());
        eprintln!(
            "[exclude] dropped {} tool(s); model sees {} of {n_before}: {:?}",
            n_before - filtered.len(),
            filtered.len(),
            args.exclude
        );
        filtered
    };
    let registry = ToolRegistry::new(tools);

    // ADR-019 audit log — JSONL beside the session file (`<id>.audit.jsonl`).
    // Captures orchestrator/tool/approval events; session log keeps the model
    // transcript. ApprovalHook also gets this writer below so approval
    // decisions are recorded under `AuditClass::Approval`.
    let audit_path = path.with_extension("audit.jsonl");
    let audit_writer =
        Arc::new(AuditWriter::create(&audit_path, session_id, TenantId::single_tenant()).await?);

    let sink: Arc<dyn EventSink> = Arc::new(FanOutSink::new(vec![
        Arc::new(StderrSink) as Arc<dyn EventSink>,
        Arc::new(writer) as Arc<dyn EventSink>,
        audit_writer.clone() as Arc<dyn EventSink>,
    ]));
    let budget: Arc<dyn LifecycleHook> = Arc::new(TurnBudgetHook::new(max_calls));

    // ADR-026 — install compaction transform unless --no-compact.
    let compaction: Option<Arc<dyn ContextTransform>> = if args.no_compact {
        None
    } else {
        let cfg = CompactionConfig {
            context_window: args
                .context_window
                .unwrap_or(DEFAULT_COMPACTION_CONFIG.context_window),
            recent_turns_to_keep: args
                .recent_turns_to_keep
                .unwrap_or(DEFAULT_COMPACTION_CONFIG.recent_turns_to_keep),
            ..DEFAULT_COMPACTION_CONFIG
        };
        Some(Arc::new(SummarizingTransform::new(
            model.clone(),
            model_id.clone(),
            cfg,
        )))
    };

    // ADR-033 — overlay the role's prompt fragment onto the agent base
    // prompt. Role guidance is always appended; agent identity stays first
    // for prefix-cache stability across role switches.
    let effective_prompt: String = match role.as_ref() {
        Some(r) => format!("{SYSTEM_PROMPT}\n\n{}", r.prompt),
        None => SYSTEM_PROMPT.to_string(),
    };

    let mut builder = AgentLoop::builder()
        .session_id(session_id)
        .system_prompt(effective_prompt)
        .model(model, model_id)
        .tools(registry)
        .resume(prior_messages)
        .sink(sink)
        .lifecycle_hook(budget)
        .max_iterations(max_calls + 4);
    if let Some(t) = compaction {
        builder = builder.context_transform(t);
    }

    // ADR-029 — install HITL approval hook unless --no-approval.
    if !args.no_approval {
        let prompter: Arc<dyn Approval> = Arc::new(CliPrompter);
        let mut hook = ApprovalHook::new(
            ApprovalPolicy::OnRequest,
            Arc::new(ToolNameMatcher::coder_defaults()),
            prompter,
        )
        .with_audit(audit_writer.clone() as Arc<dyn EventSink>);
        if let Some(aug) = mcp_risk_augmenter.clone() {
            hook = hook.with_augmenter(aug);
        }
        builder = builder.hook(Arc::new(hook));
        let aug_msg = if mcp_risk_augmenter.is_some() {
            " + MCP risk augmenter"
        } else {
            ""
        };
        eprintln!(
            "[approval] CLI prompter installed; deny-list = bash, write_file, edit_file{aug_msg}"
        );
    }
    let mut agent = builder.build()?;

    eprintln!("[sess ] {} (path: {})", session_id.0, path.display());

    let (_handle, signal) = AbortSignal::new();
    agent.run(user_text(prompt), signal).await?;

    let final_text = agent
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
                (!t.is_empty()).then_some(t)
            }
            _ => None,
        })
        .unwrap_or_else(|| "(no assistant text)".into());

    println!("{final_text}");
    Ok(())
}

/// Returns `(session_id, prior_messages, header_existed)`.
async fn resolve_session(
    tenant_root: &std::path::Path,
    cwd: &std::path::Path,
    args: &Args,
) -> anyhow::Result<(SessionId, Vec<AgentMessage>, bool)> {
    if args.r#continue {
        let entry = find_latest_by_cwd(tenant_root, cwd)
            .await?
            .ok_or_else(|| anyhow::anyhow!("--continue: no prior session in cwd {cwd:?}"))?;
        let id = entry.session_id;
        let path = tenant_root.join(format!("{}.jsonl", id.0));
        let messages = SessionReader::open(&path).await?.messages();
        Ok((id, messages, true))
    } else if let Some(handle) = &args.session {
        let entry = resolve_handle(tenant_root, handle)
            .await?
            .ok_or_else(|| anyhow::anyhow!("--session: handle '{handle}' not found"))?;
        let id = entry.session_id;
        let path = tenant_root.join(format!("{}.jsonl", id.0));
        let messages = SessionReader::open(&path).await?.messages();
        Ok((id, messages, true))
    } else {
        // Fresh session.
        Ok((SessionId::new(), Vec::new(), false))
    }
}
