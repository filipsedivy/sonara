//! The commands (`cmd_*` of `sonara.cli.main`). Output is JSON on stdout, formatted as the
//! prototype's `_print` formats it (`json.dumps(obj, indent=2)` and a newline); progress and
//! failures go to stderr.
//!
//! Exit codes: 0 success, 1 a tool or command error, 2 a usage error (`clap`, as `argparse`).
//! A domain error a command raises (the prototype's `SonaraError`, caught by `main`) prints
//! `{"ok": false, "error": {...}}` and exits with 1. Every other failure prints its message on
//! stderr and exits with 1, where the prototype ended with a traceback or `SystemExit(message)`.
//!
//! `tool` and `batch` use a running session (the GUI or `sonara serve`, over IPC) unless
//! `--connect never`, and run in-process when none runs (`--connect auto`). `gui` starts the
//! desktop application, `sonara-desktop`, which ships next to this executable.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser as _;
use serde_json::{Map, Value, json};
use sonara_agent::handlers::builtin;
use sonara_agent::ipc::{ClientContext, ClientOptions, IpcClient, IpcServer, ServerOptions};
use sonara_agent::mcp::{ToolBackend, dumps, pydantic_order, run_stdio};
use sonara_agent::registry::error_json;
use sonara_agent::service::{AgentOptions, AgentService};
use sonara_agent::state::{StateOptions, project_state};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::media::inspect_file;
use sonara_app::paths::{expand_user, resolve, runtime_dir, workspaces_dir};
use sonara_app::render::analyze_file;
use sonara_app::transport::{DevicesError, native_devices};
use sonara_core::error::DomainError;
use sonara_core::plugins::builtin_registry;
use sonara_core::pyfmt::str_repr;
use sonara_project::store::{ProjectStore, StoreError, detect_layout};

use crate::batch::{BatchError, is_ok, py_str, run_batch, truthy};
use crate::cli::{BatchArgs, Command, McpCli, RenderArgs, ServeArgs, SessionArgs, ToolArgs};
use crate::logging;

/// The prototype's `EXIT_OK`.
const EXIT_OK: u8 = 0;
/// The prototype's `EXIT_TOOL_ERROR`.
const EXIT_TOOL_ERROR: u8 = 1;

/// Why a command failed.
#[derive(Debug)]
pub(crate) enum Failure {
    /// A domain error, printed as `{"ok": false, "error": ...}`.
    Domain(DomainError),
    /// Any other failure, printed on stderr.
    Fatal(String),
}

impl Failure {
    fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    /// The store's domain errors are domain errors; its file-system failures are fatal.
    fn store(error: StoreError) -> Self {
        match error {
            StoreError::Domain(error) => Self::Domain(error),
            other => Self::Fatal(other.to_string()),
        }
    }

    /// Reports the failure and returns the exit code.
    pub(crate) fn report(self) -> ExitCode {
        match self {
            Self::Domain(error) => {
                if let Err(fatal) = print(&json!({"ok": false, "error": error_json(&error)})) {
                    return fatal.report();
                }
            }
            Self::Fatal(message) => {
                let mut stderr = io::stderr().lock();
                // Nothing is left to report to when stderr fails.
                let _ = writeln!(stderr, "sonara: {message}");
            }
        }
        ExitCode::from(EXIT_TOOL_ERROR)
    }
}

/// What a command returns.
type Outcome = Result<ExitCode, Failure>;

/// `_print`: `json.dumps(value, indent=2)` and a newline on stdout.
fn print(value: &Value) -> Result<(), Failure> {
    let mut text = dumps(value, 2);
    text.push('\n');
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::fatal(format!("cannot write the output: {error}")))
}

/// Exit 0 for an `ok` envelope, else 1.
fn exit_for(ok: bool) -> ExitCode {
    ExitCode::from(if ok { EXIT_OK } else { EXIT_TOOL_ERROR })
}

/// Runs `command`.
pub(crate) fn run(command: Command) -> Outcome {
    match command {
        Command::Gui { project } => gui(project.as_deref()),
        Command::Serve(args) => serve(&args),
        Command::Mcp { rest } => mcp(rest),
        Command::Tools { name } => tools(name.as_deref()),
        Command::Tool(args) => tool(&args),
        Command::Batch(args) => batch(&args),
        Command::Render(args) => render(&args),
        Command::Info { file } => info(&file),
        Command::Analyze { file, series } => analyze(&file, series),
        Command::Devices => devices(),
        Command::Verify { project, quick } => verify(&project, quick),
    }
}

// -- gui -------------------------------------------------------------------------------------

/// The desktop application's executable, next to this one: `Contents/MacOS` in the
/// bundle, `target/<profile>` in a build. Links to this executable are followed first, so a
/// `sonara` linked onto the `PATH` finds the bundle's.
fn desktop_executable() -> Result<PathBuf, Failure> {
    let this = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|error| Failure::fatal(format!("cannot locate the sonara executable: {error}")))?;
    let name = format!("sonara-desktop{}", std::env::consts::EXE_SUFFIX);
    let desktop = this.with_file_name(name);
    if desktop.is_file() {
        Ok(desktop)
    } else {
        Err(Failure::fatal(format!(
            "the desktop application is not installed: {} is missing",
            desktop.display()
        )))
    }
}

/// `sonara gui [PROJECT]`: runs the desktop application with the project, if any, and exits
/// with its exit code, as the prototype ran its GUI in-process. On Unix the desktop replaces
/// this process. `--log-level` is not passed on: the desktop logs at its own default level, to
/// stderr and to `logs/sonara.log`.
fn gui(project: Option<&Path>) -> Outcome {
    let desktop = desktop_executable()?;
    let mut command = std::process::Command::new(&desktop);
    command.args(project);
    let failed = |error: io::Error| {
        Failure::fatal(format!(
            "cannot start the desktop application {}: {error}",
            desktop.display()
        ))
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // `exec` returns only when it failed.
        Err(failed(command.exec()))
    }
    #[cfg(not(unix))]
    {
        let status = command.status().map_err(failed)?;
        let code = status.code().and_then(|code| u8::try_from(code).ok());
        Ok(ExitCode::from(code.unwrap_or(EXIT_TOOL_ERROR)))
    }
}

// -- sessions --------------------------------------------------------------------------------

/// An in-process session (`_local_agent`): the application and its agent service. The
/// application's jobs stop when it drops (the prototype's `finally: app.close()`).
struct Local {
    service: Arc<AgentService>,
    app: Arc<AppContext>,
}

impl Local {
    /// `AgentService(AppContext.create(AppOptions(audio_backend=audio)), base_dir=base_dir,
    /// source=source, allowed_paths=allowed)`.
    fn start(
        audio: Option<String>,
        base_dir: PathBuf,
        source: &str,
        allowed: Vec<PathBuf>,
    ) -> Result<Self, Failure> {
        let app = AppContext::create(AppOptions {
            audio_backend: audio,
            ..AppOptions::default()
        })
        .map_err(|error| Failure::fatal(error.to_string()))?;
        let app = Arc::new(app);
        let service = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(base_dir),
                source: source.to_owned(),
                allowed_paths: allowed,
                ..AgentOptions::default()
            },
        )
        .map_err(|error| Failure::fatal(error.to_string()))?;
        Ok(Self {
            service: Arc::new(service),
            app,
        })
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        self.app.close();
    }
}

/// Where tools run: in-process, or in a running session over IPC.
enum Session {
    Local(Local),
    // Boxed: the client holds its connection buffers.
    Remote(Box<IpcClient>),
}

impl Session {
    /// [`ToolBackend::call`]; a call that does not reach the tools is the prototype's
    /// `SonaraError`, caught by `main`.
    fn call(&self, name: &str, arguments: &Value) -> Result<Value, Failure> {
        ToolBackend::call(self, name, Some(arguments)).map_err(Failure::Domain)
    }
}

impl ToolBackend for Session {
    fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError> {
        match self {
            Self::Local(local) => ToolBackend::call(local.service.as_ref(), name, arguments),
            Self::Remote(client) => ToolBackend::call(client.as_ref(), name, arguments),
        }
    }

    fn tools(&self) -> Result<Vec<Value>, DomainError> {
        match self {
            Self::Local(local) => ToolBackend::tools(local.service.as_ref()),
            Self::Remote(client) => ToolBackend::tools(client.as_ref()),
        }
    }
}

/// The working directory, the base of relative paths.
fn cwd() -> Result<PathBuf, Failure> {
    std::env::current_dir()
        .map_err(|error| Failure::fatal(format!("cannot read the working directory: {error}")))
}

/// `IPCClient.connect(client=client, context={base_dir, allowed_paths})`: the newest running
/// session that accepts the client, if any.
fn attach(
    client: &str,
    base_dir: PathBuf,
    allowed: &[PathBuf],
) -> Result<Option<IpcClient>, Failure> {
    let directory = runtime_dir().map_err(|error| Failure::fatal(error.to_string()))?;
    let options = ClientOptions {
        client: client.to_owned(),
        context: Some(ClientContext {
            base_dir: Some(base_dir),
            allowed_paths: allowed.iter().map(|path| resolve(path)).collect(),
        }),
        ..ClientOptions::default()
    };
    Ok(IpcClient::connect_discovered(&directory, &options))
}

/// `_backend`: a running session unless `--connect never`; else, or when none runs and the
/// mode is `auto`, an in-process session with source `cli`, the working directory as base and
/// the `--allow` folders.
fn backend(session: &SessionArgs) -> Result<Session, Failure> {
    let base_dir = cwd()?;
    if session.connect != "never" {
        if let Some(client) = attach("sonara-cli", base_dir.clone(), &session.allow)? {
            return Ok(Session::Remote(Box::new(client)));
        }
        if session.connect == "always" {
            return Err(Failure::fatal("no running Sonara session found"));
        }
    }
    let local = Local::start(
        Some(session.audio.clone()),
        base_dir,
        "cli",
        session.allow.clone(),
    )?;
    Ok(Session::Local(local))
}

// -- tools and tool ----------------------------------------------------------------------------

/// `sonara tools [NAME]`: the registered tools, or one tool's contract entry.
fn tools(name: Option<&str>) -> Outcome {
    let registry = builtin();
    if let Some(name) = name {
        let mut spec = registry.get(name).map_err(Failure::Domain)?.to_json();
        // `ToolSpec.to_json`, with the input schema in pydantic's key order.
        if let Some(schema) = spec.get_mut("input_schema") {
            *schema = pydantic_order(schema);
        }
        print(&spec)?;
    } else {
        let listing: Vec<Value> = registry
            .specs()
            .iter()
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "title": spec.title,
                    "safety": spec.safety.as_str(),
                    "description": spec.description(),
                })
            })
            .collect();
        print(&Value::Array(listing))?;
    }
    Ok(ExitCode::from(EXIT_OK))
}

/// `_parse_kv`: `key=value` pairs, values read as JSON when they are JSON.
fn parse_pairs(pairs: &[String]) -> Result<Map<String, Value>, Failure> {
    let mut out = Map::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(Failure::fatal(format!(
                "-a expects key=value, got {}",
                str_repr(pair)
            )));
        };
        let value = serde_json::from_str(value).unwrap_or_else(|_| Value::from(value));
        out.insert(key.to_owned(), value);
    }
    Ok(out)
}

/// `sonara tool NAME [JSON] [-a key=value]...`.
fn tool(args: &ToolArgs) -> Outcome {
    let mut payload = match &args.json {
        Some(text) => match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => map,
            Ok(_) => return Err(Failure::fatal("the JSON arguments must be an object")),
            Err(error) => {
                return Err(Failure::fatal(format!(
                    "the JSON arguments are not valid JSON: {error}"
                )));
            }
        },
        None => Map::new(),
    };
    payload.extend(parse_pairs(&args.arg)?);
    let session = backend(&args.session)?;
    if let Some(project) = &args.project {
        let opened = session.call("open_project", &json!({"path": project}))?;
        if !is_ok(&opened) {
            print(&opened)?;
            return Ok(ExitCode::from(EXIT_TOOL_ERROR));
        }
    }
    let mut result = session.call(&args.name, &Value::Object(payload))?;
    if args.save && is_ok(&result) {
        let saved = session.call("save_project", &json!({}))?;
        if let Value::Object(map) = &mut result {
            map.insert("saved".to_owned(), saved);
        }
    }
    print(&result)?;
    Ok(exit_for(is_ok(&result)))
}

// -- batch -------------------------------------------------------------------------------------

/// `sonara batch FILE`: the steps of `{"steps": [...]}` or of a plain list.
fn batch(args: &BatchArgs) -> Outcome {
    let text = fs::read_to_string(&args.file)
        .map_err(|error| Failure::fatal(format!("cannot read {}: {error}", args.file.display())))?;
    let spec: Value = serde_json::from_str(&text).map_err(|error| {
        Failure::fatal(format!(
            "{} is not valid JSON: {error}",
            args.file.display()
        ))
    })?;
    let steps = match &spec {
        Value::Object(map) => map
            .get("steps")
            .ok_or_else(|| Failure::fatal("the batch has no \"steps\""))?,
        other => other,
    };
    let Value::Array(steps) = steps else {
        return Err(Failure::fatal("the batch steps are not a list"));
    };
    let session = backend(&args.session)?;
    let mut full = Map::new();
    let mut log = |sid: &Value, tool: &str, res: &Value| {
        let status = if is_ok(res) {
            "ok".to_owned()
        } else {
            let field = |key: &str| py_str(res.pointer(key).unwrap_or(&Value::Null));
            format!(
                "ERROR {}: {}",
                field("/error/code"),
                field("/error/message")
            )
        };
        let mut stderr = io::stderr().lock();
        // Progress lines are best effort, as the prototype's `sys.stderr.write`.
        let _ = writeln!(stderr, "[{}] {tool}: {status}", py_str(sid));
    };
    let outputs =
        run_batch(&session, steps, !args.keep_going, &mut full, &mut log).map_err(|error| {
            match error {
                BatchError::Call(error) => Failure::Domain(error),
                other => Failure::fatal(other.to_string()),
            }
        })?;
    if let Some(path) = &args.results {
        let mut text = dumps(&Value::Object(full), 1);
        text.push('\n');
        fs::write(path, text)
            .map_err(|error| Failure::fatal(format!("cannot write {}: {error}", path.display())))?;
    }
    let all_ok = outputs
        .iter()
        .all(|output| output.get("ok").is_some_and(truthy));
    print(&json!({"steps": outputs, "ok": all_ok}))?;
    Ok(exit_for(all_ok))
}

// -- render ------------------------------------------------------------------------------------

/// `sonara render PROJECT -o OUT`: opens the project and exports it with `export_audio`,
/// allowing the project's and the output's folders.
fn render(args: &RenderArgs) -> Outcome {
    let base = cwd()?;
    let project = resolve(&args.project);
    let output = resolve(&args.output);
    let allowed = [project.parent(), output.parent()]
        .into_iter()
        .flatten()
        .map(Path::to_path_buf)
        .collect();
    let session = Session::Local(Local::start(Some("null".to_owned()), base, "cli", allowed)?);
    let opened = session.call("open_project", &json!({"path": project.to_string_lossy()}))?;
    if !is_ok(&opened) {
        print(&opened)?;
        return Ok(ExitCode::from(EXIT_TOOL_ERROR));
    }
    let wav = ["wav".to_owned()];
    let formats: Vec<Value> = args
        .format
        .as_deref()
        .unwrap_or(&wav)
        .iter()
        .map(|format| json!({"format": format, "bit_depth": args.bit_depth}))
        .collect();
    let mut export = json!({
        "path": output.to_string_lossy(),
        "formats": formats,
        "normalize": args.normalize,
        "target_db": args.target,
        "overwrite": args.overwrite,
        "confirmation_token": null,
    });
    let mut result = session.call("export_audio", &export)?;
    if !is_ok(&result)
        && result.pointer("/error/code").and_then(Value::as_str) == Some("CONFIRMATION_REQUIRED")
        && args.overwrite
    {
        export["confirmation_token"] = result
            .pointer("/error/details/confirmation_token")
            .cloned()
            .unwrap_or(Value::Null);
        result = session.call("export_audio", &export)?;
    }
    print(&result)?;
    Ok(exit_for(is_ok(&result)))
}

// -- info, analyze, devices, verify ------------------------------------------------------------

/// The store of the application's workspaces (`ProjectStore(workspaces_dir())`).
fn store() -> Result<ProjectStore, Failure> {
    let workspaces = workspaces_dir().map_err(|error| Failure::fatal(error.to_string()))?;
    Ok(ProjectStore::new(workspaces))
}

/// `sonara info FILE`: a project's layout, warnings and state, or an audio file's inspection.
fn info(file: &Path) -> Outcome {
    let path = expand_user(file);
    if detect_layout(&path).is_some() {
        let loaded = store()?.open(&path, false).map_err(Failure::store)?;
        let plugins = builtin_registry();
        let state = project_state(&loaded.project, &plugins, StateOptions::default());
        print(&json!({
            "layout": loaded.layout.as_str(),
            "warnings": loaded.warnings,
            "state": state,
        }))?;
    } else {
        print(&inspect_file(&path).map_err(Failure::Domain)?.to_json())?;
    }
    Ok(ExitCode::from(EXIT_OK))
}

/// `sonara analyze FILE`: the analyzer's report of the decoded file, fed in 65536-frame float64
/// blocks as the prototype's `cmd_analyze` fed it, and its issues. A file that cannot be read
/// fails as the prototype's `soundfile` error did.
fn analyze(file: &Path, series: bool) -> Outcome {
    let report = analyze_file(file, series).map_err(|error| Failure::fatal(error.to_string()))?;
    print(&report)?;
    Ok(ExitCode::from(EXIT_OK))
}

/// `sonara devices`: the native backend's devices (`{"backend": "native/<name>", "devices":
/// [{"id", "name", "kind", "default", "channels", "sample_rates"}]}`), or `{"available": false,
/// "reason"}` with exit 1 when the system's audio context cannot be created.
fn devices() -> Outcome {
    match native_devices() {
        Ok(listing) => {
            print(&listing)?;
            Ok(ExitCode::from(EXIT_OK))
        }
        Err(DevicesError::Unavailable(reason)) => {
            print(&json!({"available": false, "reason": reason}))?;
            Ok(ExitCode::from(EXIT_TOOL_ERROR))
        }
        Err(error @ DevicesError::Enumeration(_)) => Err(Failure::fatal(error.to_string())),
    }
}

/// `sonara verify PROJECT`: the integrity report, with the warnings of opening the project.
fn verify(project: &Path, quick: bool) -> Outcome {
    let store = store()?;
    let loaded = store.open(project, false).map_err(Failure::store)?;
    let report = store
        .verify(&loaded.project, &loaded.workspace, !quick)
        .map_err(Failure::store)?;
    let mut out = Map::new();
    out.insert("warnings".to_owned(), json!(loaded.warnings));
    if let Value::Object(fields) = report.to_json() {
        out.extend(fields);
    }
    print(&Value::Object(out))?;
    Ok(exit_for(report.ok))
}

// -- serve and mcp -----------------------------------------------------------------------------

/// `sonara serve`: a headless session with the IPC server, autosaving every second while the
/// project has unsaved changes, until SIGINT or SIGTERM.
fn serve(args: &ServeArgs) -> Outcome {
    let local = Local::start(args.audio.clone(), cwd()?, "ipc", args.allow.clone())?;
    if let Some(project) = &args.project {
        let path = resolve(project);
        let opened = local.service.call(
            "open_project",
            Some(&json!({"path": path.to_string_lossy()})),
        );
        if !is_ok(&opened) {
            print(&opened)?;
            return Ok(ExitCode::from(EXIT_TOOL_ERROR));
        }
    }
    let server = IpcServer::start(
        Arc::clone(&local.service),
        ServerOptions {
            app_kind: "headless".to_owned(),
            directory: None,
        },
    )
    .map_err(|error| Failure::fatal(error.to_string()))?;
    {
        let mut stderr = io::stderr().lock();
        // A status line, best effort as the prototype's `sys.stderr.write`.
        let _ = writeln!(
            stderr,
            "sonara serve: listening on {} (discovery {}); Ctrl+C to stop",
            server.address(),
            server.discovery_path().display()
        );
    }
    let outcome = until_stopped(|| {
        local
            .app
            .projects()
            .autosave()
            .map(|_| ())
            .map_err(|error| Failure::fatal(format!("autosave failed: {error}")))
    });
    server.close();
    outcome?;
    Ok(ExitCode::from(EXIT_OK))
}

/// Calls `tick` every second until SIGINT or (on Unix) SIGTERM arrives; a failing `tick` ends
/// the wait with its failure.
fn until_stopped(mut tick: impl FnMut() -> Result<(), Failure>) -> Result<(), Failure> {
    let signals = |error: io::Error| Failure::fatal(format!("cannot watch for signals: {error}"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(signals)?;
    runtime.block_on(async {
        #[cfg(unix)]
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(signals)?;
        let interrupt = tokio::signal::ctrl_c();
        tokio::pin!(interrupt);
        loop {
            #[cfg(unix)]
            let terminated = terminate.recv();
            #[cfg(not(unix))]
            let terminated = std::future::pending::<Option<()>>();
            tokio::select! {
                stopped = &mut interrupt => return stopped.map_err(signals),
                _ = terminated => return Ok(()),
                () = tokio::time::sleep(Duration::from_secs(1)) => tick()?,
            }
        }
    })
}

/// `sonara mcp [ARGS]...`: the MCP server on stdio (`sonara-mcp ARGS`), attached to a running
/// session unless `--connect never`, else headless.
fn mcp(rest: Vec<OsString>) -> Outcome {
    let args = std::iter::once(OsString::from("sonara-mcp")).chain(rest);
    let cli = match McpCli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            // Usage errors exit with 2, `--help` with 0, as `argparse` does.
            let code = error.exit_code();
            let _ = error.print();
            return Ok(ExitCode::from(u8::try_from(code).unwrap_or(2)));
        }
    };
    logging::init(&cli.log_level).map_err(Failure::Fatal)?;
    let base_dir = match cli.base_dir {
        Some(dir) => dir,
        None => cwd()?,
    };
    let mut session = None;
    if cli.connect != "never" {
        if let Some(client) = attach("sonara-mcp", resolve(&base_dir), &cli.allow)? {
            log::warn!(
                target: "sonara.mcp",
                "sonara-mcp ready: connected to running Sonara ({}) at {}",
                client.discovery().app,
                client.discovery().address
            );
            session = Some(Session::Remote(Box::new(client)));
        } else if cli.connect == "always" {
            return Err(Failure::fatal(
                "no running Sonara session found (start the GUI or `sonara serve`)",
            ));
        }
    }
    let session = if let Some(session) = session {
        session
    } else {
        let local = Local::start(cli.audio, base_dir, "agent", cli.allow)?;
        log::warn!(target: "sonara.mcp", "sonara-mcp ready: headless in-process session");
        Session::Local(local)
    };
    let backend: Arc<dyn ToolBackend> = Arc::new(session);
    run_stdio(backend).map_err(|error| Failure::fatal(error.to_string()))?;
    Ok(ExitCode::from(EXIT_OK))
}
