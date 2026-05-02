//! OS service / daemon support.
//!
//! - On Windows, registers the supervisor as a service named `MnemeDaemon`
//!   via the `windows-service` crate. The service entry point bridges into
//!   [`crate::run`].
//! - On Unix, provides a minimal `daemonize` fallback (double-fork + setsid)
//!   so the supervisor can detach from a controlling terminal.

use crate::config::SupervisorConfig;
use crate::error::SupervisorError;
use tracing::info;

/// Service installation / runtime mode requested by the CLI.
#[derive(Debug, Clone, Copy)]
pub enum ServiceAction {
    /// Run inline in the current console session (no detach).
    RunForeground,
    /// Detach (Unix daemonize) or run as Windows service.
    RunAsService,
    /// Install as a service (Windows only — Unix is a no-op).
    Install,
    /// Uninstall the service (Windows only — Unix is a no-op).
    Uninstall,
}

/// Cross-platform entry point.
pub async fn execute(
    action: ServiceAction,
    config: SupervisorConfig,
) -> Result<(), SupervisorError> {
    match action {
        ServiceAction::RunForeground => crate::run(config).await,
        ServiceAction::RunAsService => run_as_service(config).await,
        ServiceAction::Install => install_service(),
        ServiceAction::Uninstall => uninstall_service(),
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Notify;
    use windows_service::service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_dispatcher};

    pub const SERVICE_NAME: &str = "MnemeDaemon";
    pub const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        // NEW-013: SCM expects a "Running" status within ~30s of dispatch
        // or it kills the worker with "did not respond in time". Register
        // the control handler and signal RUNNING IMMEDIATELY — before the
        // (potentially slow / failure-prone) config load, before runtime
        // build, before any worker spawn. The actual supervisor work then
        // runs underneath an already-Running service, the way SCM wants.

        let shutdown = Arc::new(Notify::new());
        let shutdown_for_handler = shutdown.clone();

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    shutdown_for_handler.notify_waiters();
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
            Ok(h) => h,
            Err(_) => return,
        };

        // First: tell SCM we are RUNNING. Idempotent — `crate::run` does its
        // own logging and shutdown notify; SCM only cares that we acked.
        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });

        // Now (already Running from SCM's POV) load config. Same fallback
        // discipline as `Cmd::ServiceRun` in main.rs: a corrupt or missing
        // file degrades to default_layout instead of taking the service
        // down with a "did not respond" error.
        let cfg = match SupervisorConfig::load(std::path::Path::new(&default_config_path())) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "service_main: config load failed; using default_layout");
                SupervisorConfig::default_layout()
            }
        };

        // I-4 / I-5 / NEW-008: cap the runtime for the same reason main.rs
        // does — even under SCM the supervisor is a control-plane daemon,
        // not a compute pool. Keeping the thread count bounded gives us a
        // predictable RSS floor.
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get().clamp(1, 4))
            .max_blocking_threads(8)
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };

        // Bug G-3 (2026-05-01): capture the supervisor's return value
        // and report it to the SCM. Previously this was
        //     let _ = crate::run(cfg).await;
        // … with `exit_code: ServiceExitCode::Win32(0)` hardcoded
        // unconditionally below. That meant a crashed supervisor
        // showed up in `services.msc` as "Stopped — success" and
        // every CLI command silently hung at the IPC connect because
        // nothing was actually listening. Now, on Err we set the
        // exit_code to a service-specific non-zero so SCM, Event
        // Viewer, and `sc query MnemeDaemon` all reflect reality.
        let run_result = rt.block_on(async { crate::run(cfg).await });
        let exit_code = match &run_result {
            Ok(()) => ServiceExitCode::Win32(0),
            Err(e) => {
                tracing::error!(error = %e, "supervisor run() returned error inside Windows service wrapper");
                // 1u32 is a generic "service-specific failure" marker —
                // the real diagnostic is in the supervisor.log written
                // before this point.
                ServiceExitCode::ServiceSpecific(1)
            }
        };

        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code,
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });
    }

    pub fn run_dispatcher() -> Result<(), SupervisorError> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
            .map_err(|e| SupervisorError::Other(format!("service dispatcher failed: {e}")))
    }

    pub fn install() -> Result<(), SupervisorError> {
        let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
            .map_err(|e| SupervisorError::Other(format!("open scm: {e}")))?;
        let exe_path = std::env::current_exe()
            .map_err(|e| SupervisorError::Other(format!("current_exe: {e}")))?;
        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from("Mneme Daemon"),
            service_type: SERVICE_TYPE,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe_path,
            launch_arguments: vec![OsString::from("service-run")],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };
        service_manager
            .create_service(&info, ServiceAccess::CHANGE_CONFIG)
            .map_err(|e| SupervisorError::Other(format!("create_service: {e}")))?;
        Ok(())
    }

    pub fn uninstall() -> Result<(), SupervisorError> {
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
            .map_err(|e| SupervisorError::Other(format!("open scm: {e}")))?;
        let access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
        let svc = service_manager
            .open_service(SERVICE_NAME, access)
            .map_err(|e| SupervisorError::Other(format!("open_service: {e}")))?;
        let _ = svc.stop();
        svc.delete()
            .map_err(|e| SupervisorError::Other(format!("delete service: {e}")))?;
        Ok(())
    }

    fn default_config_path() -> String {
        std::env::var("MNEME_CONFIG").unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
            format!("{home}\\.mneme\\supervisor.toml")
        })
    }
}

#[cfg(windows)]
async fn run_as_service(_config: SupervisorConfig) -> Result<(), SupervisorError> {
    // Hand control to the Windows service dispatcher. Note: the dispatcher is
    // *blocking*; tokio::task::spawn_blocking would prevent re-entry, so we
    // call it directly.
    info!("starting as Windows service");
    windows_impl::run_dispatcher()
}

#[cfg(windows)]
fn install_service() -> Result<(), SupervisorError> {
    info!("installing Windows service");
    windows_impl::install()
}

#[cfg(windows)]
fn uninstall_service() -> Result<(), SupervisorError> {
    info!("uninstalling Windows service");
    windows_impl::uninstall()
}

#[cfg(unix)]
async fn run_as_service(config: SupervisorConfig) -> Result<(), SupervisorError> {
    info!("starting as Unix daemon");
    daemonize()?;
    crate::run(config).await
}

#[cfg(unix)]
fn install_service() -> Result<(), SupervisorError> {
    info!("install_service is a no-op on Unix; use systemd / launchd manually");
    Ok(())
}

#[cfg(unix)]
fn uninstall_service() -> Result<(), SupervisorError> {
    info!("uninstall_service is a no-op on Unix; use systemd / launchd manually");
    Ok(())
}

/// Minimal double-fork daemonization.
///
/// In addition to the classic detach (fork → setsid → fork → chdir/umask),
/// we redirect fds 0/1/2 to `/dev/null` (WIDE-009). Without this the
/// daemon inherits whatever stdio the launching shell had — closing the
/// shell can then deliver SIGHUP/SIGPIPE to the daemon if it writes, and
/// any leftover pipes pin the parent's TTY. Same fix every well-behaved
/// Unix daemon does (see Stevens APUE §13.3 and `daemon(3)` source).
#[cfg(unix)]
fn daemonize() -> Result<(), SupervisorError> {
    use nix::sys::stat::{umask, Mode};
    use nix::unistd::{chdir, dup2, fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;

    // First fork.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => std::process::exit(0),
        Ok(ForkResult::Child) => {}
        Err(e) => return Err(SupervisorError::Other(format!("first fork failed: {e}"))),
    }

    setsid().map_err(|e| SupervisorError::Other(format!("setsid failed: {e}")))?;

    // Second fork.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => std::process::exit(0),
        Ok(ForkResult::Child) => {}
        Err(e) => return Err(SupervisorError::Other(format!("second fork failed: {e}"))),
    }

    umask(Mode::empty());
    chdir("/").map_err(|e| SupervisorError::Other(format!("chdir failed: {e}")))?;

    // WIDE-009: replace stdin/stdout/stderr with /dev/null. We open the
    // device read-write so a single fd can back all three; dup2 then
    // atomically replaces fds 0/1/2 (closing the previous descriptor).
    // Failures are non-fatal — a degraded daemon with original stdio is
    // still safer than one that aborts startup.
    let null = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| SupervisorError::Other(format!("open /dev/null failed: {e}")))?;
    let nullfd = null.as_raw_fd();
    for target in 0..=2 {
        if let Err(e) = dup2(nullfd, target) {
            return Err(SupervisorError::Other(format!(
                "dup2 /dev/null -> fd {target} failed: {e}"
            )));
        }
    }
    // `null` goes out of scope here. The OpenOptions handle is closed,
    // but fds 0/1/2 are independent dup2 copies so they survive.
    drop(null);

    Ok(())
}
