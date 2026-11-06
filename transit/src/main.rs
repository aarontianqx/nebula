mod adapter;
mod admission;
mod audit;
mod capture;
mod config;
mod dashboard;
mod error;
mod logging;
mod observation;
mod observer;
mod privacy;
mod proxy;
mod server;

use clap::{Parser, Subcommand};
use config::{Config, Snapshot};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

#[derive(Parser)]
#[command(
    version,
    about = "Streaming model API proxy. Credentials stay with the caller."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Check {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("transit: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let (serve, path) = match cli.command {
        Command::Serve { config } => (true, config),
        Command::Check { config } => (false, config),
    };
    let path = match path {
        Some(path) => {
            std::path::absolute(path).map_err(|e| format!("cannot resolve config path: {e}"))?
        }
        None => config::default_config_path()?,
    };
    let initial = Config::load(&path)?;
    if !serve {
        println!(
            "Configuration valid: {} (default provider: {}; providers: {}; routes: {})",
            path.display(),
            initial.default_provider,
            initial.providers.len(),
            initial.routes.len()
        );
        return Ok(());
    }
    let (logs, log_guard) = logging::initialize(&initial.logging)?;
    let directory = logs.directory.clone();
    std::panic::set_hook(Box::new(|info| {
        if let Some(location) = info.location() {
            eprintln!(
                "transit panic: {}:{} (payload omitted)",
                location.file(),
                location.line()
            );
            tracing::error!(
                event = "panic",
                file = location.file(),
                line = location.line()
            );
        } else {
            eprintln!("transit panic (payload omitted)");
        }
    }));
    let audit_settings = initial.audit.clone();
    let audit_capacity = initial.server.max_in_flight;
    let storage = tokio::task::spawn_blocking(move || audit::open(&audit_settings, audit_capacity))
        .await
        .map_err(|_| "audit initialization task panicked")??;
    let (audit, audit_guard) = match storage {
        Some((audit, guard)) => (Some(audit), Some(guard)),
        None => (None, None),
    };
    let (observations, observation_guard) = if initial.audit.enabled {
        let (pool, guard) = observation::initialize(&initial.audit)?;
        (Some(pool), Some(guard))
    } else {
        (None, None)
    };
    let (capture, capture_guard) = capture::initialize(&initial.capture)?;
    let listener = tokio::net::TcpListener::bind(initial.server.listen)
        .await
        .map_err(|e| {
            tracing::error!(event="listener_bind_failed",listen=%initial.server.listen,error_kind=?e.kind());
            format!("cannot bind {}: {e}", initial.server.listen)
        })?;
    let address = listener
        .local_addr()
        .map_err(|e| format!("cannot read listener address: {e}"))?;
    let grace = Duration::from_secs(initial.server.shutdown_grace_seconds);
    let (dashboard_server, dashboard_guard) = if initial.dashboard.enabled {
        let listener = tokio::net::TcpListener::bind(initial.dashboard.listen)
            .await
            .map_err(|e| format!("cannot bind dashboard {}: {e}", initial.dashboard.listen))?;
        let (reader, guard) = dashboard::initialize(&initial.audit)?;
        tracing::info!(event="dashboard_started",listen=%initial.dashboard.listen);
        eprintln!(
            "transit dashboard listening on {} (read-only)",
            initial.dashboard.listen
        );
        (Some((listener, dashboard::router(reader))), Some(guard))
    } else {
        (None, None)
    };
    let app = proxy::App::new(
        Snapshot {
            generation: 1,
            config: initial,
        },
        logs.clone(),
        audit,
        observations,
        capture,
    );
    tracing::info!(event="startup",config_path=%path.display(),log_directory=%directory.display(),listen=%address,config_generation=1,default_provider=%app.snapshot.read().unwrap_or_else(|e|e.into_inner()).config.default_provider);
    eprintln!(
        "transit listening on {address}; config: {}; logs: {} (UTC daily JSONL)",
        path.display(),
        directory.display()
    );
    let maintenance_app = app.clone();
    let mut maintenance = tokio::spawn(async move { maintenance(maintenance_app).await });
    let signal_app = app.clone();
    let mut signals = tokio::spawn(async move { signals(signal_app, path).await });
    let server_app = app.clone();
    let mut server = tokio::spawn(async move { server::serve(listener, server_app).await });
    let stopping = app.stopping.clone();
    let force_stop = app.force_stop.clone();
    let mut dashboard = tokio::spawn(async move {
        match dashboard_server {
            Some((listener, router)) => {
                server::serve_router(listener, router, stopping, force_stop, 64).await
            }
            None => {
                stopping.cancelled().await;
                Ok(())
            }
        }
    });
    let mut dashboard_finished = false;
    let mut server_finished = false;
    let mut result = Ok(());
    tokio::select! {
        response=&mut server=>{server_finished=true;match response {Ok(Ok(()))=>{},Ok(Err(error))=>result=Err(format!("HTTP server failed: {error}")),Err(_)=>result=Err("HTTP server task panicked".into())}app.stopping.cancel();},
        finished=&mut maintenance=>{if !app.stopping.is_cancelled(){tracing::error!(event="maintenance_task_exited",panicked=finished.is_err());result=Err("log maintenance task exited unexpectedly".into());}app.stopping.cancel();},
        finished=&mut signals=>{if let Ok(Err(error))=finished {result=Err(error);}else if finished.is_err(){result=Err("signal task panicked".into());}app.stopping.cancel();},
        finished=&mut dashboard=>{dashboard_finished=true;if !app.stopping.is_cancelled(){tracing::error!(event="dashboard_server_exited",panicked=finished.is_err());result=Err("dashboard server exited unexpectedly".into());}app.stopping.cancel();},
    }
    tracing::info!(
        event = "shutdown_started",
        active_requests = app.active.load(Ordering::Relaxed),
        grace_seconds = grace.as_secs()
    );
    if !server_finished {
        match tokio::time::timeout(grace, &mut server).await {
            Ok(Ok(Ok(()))) => {}
            Ok(_) => {
                result = Err("HTTP server failed during shutdown".into());
            }
            Err(_) => {
                tracing::warn!(
                    event = "shutdown_deadline",
                    active_requests = app.active.load(Ordering::Relaxed)
                );
                app.force_stop.cancel();
                if tokio::time::timeout(Duration::from_secs(2), &mut server)
                    .await
                    .is_err()
                {
                    server.abort();
                    let _ = server.await;
                }
            }
        }
    }
    app.force_stop.cancel();
    if !dashboard_finished
        && tokio::time::timeout(Duration::from_secs(5), &mut dashboard)
            .await
            .is_err()
    {
        dashboard.abort();
        let _ = dashboard.await;
    }
    if let Some(guard) = dashboard_guard {
        guard.shutdown().await;
    }
    // Connection task cancellation may finish on the next runtime turn. Flush terminal
    // audit updates only after those request guards have had a chance to run.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while app.active.load(Ordering::Relaxed) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if !maintenance.is_finished() {
        let _ = (&mut maintenance).await;
    }
    if !signals.is_finished() {
        signals.abort();
        let _ = signals.await;
    }
    capture_guard.shutdown().await;
    if let Some(guard) = observation_guard {
        guard.shutdown().await;
    }
    if let Some(guard) = audit_guard {
        guard.shutdown().await;
    }
    app.gate.report();
    logs.check_drops();
    tracing::info!(
        event = "shutdown_complete",
        active_requests = app.active.load(Ordering::Relaxed)
    );
    if let Err(ref error) = result {
        tracing::error!(event="service_failed",message=%error);
    }
    drop(app);
    drop(logs);
    drop(log_guard);
    result
}
async fn maintenance(app: Arc<proxy::App>) {
    let period = app
        .logs
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .cleanup_interval_seconds;
    let logs = app.logs.clone();
    if let Err(error) = tokio::task::spawn_blocking(move || logs.sweep()).await {
        tracing::error!(event = "janitor_panicked", cancelled = error.is_cancelled());
        return;
    }
    let mut cleanup = tokio::time::interval(Duration::from_secs(period));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    cleanup.tick().await;
    let mut health = tokio::time::interval(Duration::from_secs(10));
    health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _=app.stopping.cancelled()=>break,
            _=health.tick()=>{app.logs.check_drops();app.gate.report();app.capture.maintain();if let Some(audit)=&app.audit{audit.maintain();}tracing::debug!(event="logging_heartbeat",active_requests=app.active.load(Ordering::Relaxed));},
            _=cleanup.tick()=>{let logs=app.logs.clone();if tokio::task::spawn_blocking(move||logs.sweep()).await.is_err(){tracing::error!(event="janitor_panicked");return}},
        }
    }
}
#[cfg(unix)]
async fn signals(app: Arc<proxy::App>, path: PathBuf) -> Result<(), String> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut hangup =
        signal(SignalKind::hangup()).map_err(|e| format!("cannot register SIGHUP: {e}"))?;
    let mut terminate =
        signal(SignalKind::terminate()).map_err(|e| format!("cannot register SIGTERM: {e}"))?;
    let mut interrupt =
        signal(SignalKind::interrupt()).map_err(|e| format!("cannot register SIGINT: {e}"))?;
    loop {
        tokio::select! {
            _=terminate.recv()=>{app.stopping.cancel();return Ok(())},
            _=interrupt.recv()=>{app.stopping.cancel();return Ok(())},
            _=hangup.recv()=>{
                let path=path.clone();
                match tokio::task::spawn_blocking(move||Config::load(&path)).await {
                    Ok(Ok(config))=>{
                        let mut current=app.snapshot.write().unwrap_or_else(|e|e.into_inner());
                        if config.server!=current.config.server || config.dashboard!=current.config.dashboard || config.audit!=current.config.audit || config.capture.directory!=current.config.capture.directory || config.logging.restart_settings()!=current.config.logging.restart_settings(){tracing::warn!(event="reload_rejected",config_generation=current.generation,reason="server, dashboard, audit, capture directory and logging infrastructure settings require restart");continue}
                        let generation=current.generation+1;
                        *app.capture.settings.write().unwrap_or_else(|e|e.into_inner())=config.capture.clone();
                        *app.logs.settings.write().unwrap_or_else(|e|e.into_inner())=config.logging.clone();
                        *current=Arc::new(Snapshot{generation,config});
                        tracing::info!(event="reload_completed",config_generation=generation,default_provider=%current.config.default_provider);
                    },
                    Ok(Err(error))=>tracing::warn!(event="reload_failed",message=%error),
                    Err(error)=>tracing::error!(event="reload_task_failed",cancelled=error.is_cancelled()),
                }
            }
        }
    }
}
#[cfg(not(unix))]
async fn signals(app: Arc<proxy::App>, _: PathBuf) -> Result<(), String> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| format!("signal handling failed: {e}"))?;
    app.stopping.cancel();
    Ok(())
}
