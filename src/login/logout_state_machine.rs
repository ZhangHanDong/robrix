use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use makepad_widgets::{Cx, log, makepad_futures::channel::oneshot};

use crate::{
    home::main_desktop_ui::MainDesktopUiAction,
    persistent_state::delete_latest_user_id,
    sliding_sync::{get_client, get_sync_service, CLIENT, SYNC_SERVICE, TOMBSTONED_ROOMS, 
                   IGNORED_USERS, ALL_JOINED_ROOMS, REQUEST_SENDER, LOGOUT_POINT_OF_NO_RETURN,
                   LOGOUT_IN_PROGRESS, shutdown_background_tasks, start_matrix_tokio},
};
use super::logout_confirm_modal::{LogoutAction, MissingComponentType};
use super::logout_errors::{LogoutError, RecoverableError, UnrecoverableError};

/// Represents the current state of the logout process
#[derive(Debug, Clone, PartialEq)]
pub enum LogoutState {
    /// Initial state before logout starts
    Idle,
    /// Checking prerequisites (client, sync service existence)
    PreChecking,
    /// Stopping the sync service
    StoppingSyncService,
    /// Performing server-side logout
    LoggingOutFromServer,
    /// Reached point of no return - session invalidated
    PointOfNoReturn,
    /// Closing UI tabs (desktop only)
    ClosingTabs,
    /// Cleaning up application state
    CleaningAppState,
    /// Shutting down background tasks
    ShuttingDownTasks,
    /// Restarting the Matrix runtime
    RestartingRuntime,
    /// Logout completed successfully
    Completed,
    /// Logout failed with error
    Failed(LogoutError),
}

/// Progress information for logout operations
#[derive(Debug, Clone)]
pub struct LogoutProgress {
    pub state: LogoutState,
    pub message: String,
    pub percentage: u8,
    pub started_at: Instant,
    pub step_started_at: Instant,
}

impl LogoutProgress {
    fn new(state: LogoutState, message: String, percentage: u8) -> Self {
        let now = Instant::now();
        Self {
            state,
            message,
            percentage,
            started_at: now,
            step_started_at: now,
        }
    }
    
    fn update(&mut self, state: LogoutState, message: String, percentage: u8) {
        self.state = state;
        self.message = message;
        self.percentage = percentage;
        self.step_started_at = Instant::now();
    }
}

/// Configuration for logout process
#[derive(Debug, Clone)]
pub struct LogoutConfig {
    /// Timeout for closing tabs
    pub tab_close_timeout: Duration,
    /// Timeout for cleaning app state
    pub app_state_cleanup_timeout: Duration,
    /// Timeout for server logout
    pub server_logout_timeout: Duration,
    /// Whether to allow cancellation before point of no return
    pub allow_cancellation: bool,
    /// Whether this is desktop mode
    pub is_desktop: bool,
}

impl Default for LogoutConfig {
    fn default() -> Self {
        Self {
            tab_close_timeout: Duration::from_secs(10),
            app_state_cleanup_timeout: Duration::from_secs(5),
            server_logout_timeout: Duration::from_secs(60),
            allow_cancellation: true,
            is_desktop: true,
        }
    }
}

/// State machine for managing the logout process
pub struct LogoutStateMachine {
    current_state: Arc<Mutex<LogoutState>>,
    progress: Arc<Mutex<LogoutProgress>>,
    config: LogoutConfig,
    point_of_no_return: Arc<AtomicBool>,
    cancellation_requested: Arc<AtomicBool>,
}

impl LogoutStateMachine {
    pub fn new(config: LogoutConfig) -> Self {
        let initial_progress = LogoutProgress::new(
            LogoutState::Idle,
            "Ready to logout".to_string(),
            0
        );
        
        Self {
            current_state: Arc::new(Mutex::new(LogoutState::Idle)),
            progress: Arc::new(Mutex::new(initial_progress)),
            config,
            point_of_no_return: Arc::new(AtomicBool::new(false)),
            cancellation_requested: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Get current state
    pub async fn current_state(&self) -> LogoutState {
        self.current_state.lock().await.clone()
    }
    
    /// Get current progress
    pub async fn progress(&self) -> LogoutProgress {
        self.progress.lock().await.clone()
    }
    
    /// Request cancellation (only works before point of no return)
    pub fn request_cancellation(&self) {
        if !self.point_of_no_return.load(Ordering::Acquire) {
            self.cancellation_requested.store(true, Ordering::Release);
        }
    }
    
    /// Check if cancellation was requested
    fn is_cancelled(&self) -> bool {
        self.cancellation_requested.load(Ordering::Acquire)
    }
    
    /// Transition to a new state
    async fn transition_to(&self, new_state: LogoutState, message: String, percentage: u8) -> Result<()> {
        // Check for cancellation before transitioning
        if self.is_cancelled() && !matches!(new_state, LogoutState::PointOfNoReturn | LogoutState::Failed(_)) {
            let mut state = self.current_state.lock().await;
            *state = LogoutState::Failed(LogoutError::Recoverable(RecoverableError::Cancelled));
            return Err(anyhow!("Logout cancelled by user"));
        }
        
        log!("Logout state transition: {:?} -> {:?}", self.current_state.lock().await.clone(), new_state);
        
        let progress_message;
        let progress_percentage;
        
        {
            let mut state = self.current_state.lock().await;
            let mut progress = self.progress.lock().await;
            
            *state = new_state.clone();
            progress.update(new_state, message.clone(), percentage);
            
            progress_message = progress.message.clone();
            progress_percentage = progress.percentage;
        } // Release locks here
        
        // Send progress update to UI
        log!("Sending progress update: {} ({}%)", progress_message, progress_percentage);
        Cx::post_action(LogoutAction::ProgressUpdate { 
            message: progress_message,
            percentage: progress_percentage
        });
        
        Ok(())
    }
    
    /// Execute the logout process
    pub async fn execute(&self) -> Result<()> {
        log!("LogoutStateMachine::execute() started");
        
        // Set logout in progress flag
        LOGOUT_IN_PROGRESS.store(true, Ordering::Relaxed);
        
        // Reset global point of no return flag
        LOGOUT_POINT_OF_NO_RETURN.store(false, Ordering::Relaxed);
        
        // Start from Idle state
        self.transition_to(
            LogoutState::PreChecking,
            "Checking prerequisites...".to_string(),
            10
        ).await?;
        
        // Pre-checks
        if let Err(e) = self.perform_prechecks().await {
            self.transition_to(
                LogoutState::Failed(e.clone()),
                format!("Precheck failed: {}", e),
                0
            ).await?;
            self.handle_error(&e).await;
            return Err(anyhow!(e));
        }
        
        // Stop sync service
        self.transition_to(
            LogoutState::StoppingSyncService,
            "Stopping sync service...".to_string(),
            20
        ).await?;
        
        if let Err(e) = self.stop_sync_service().await {
            self.transition_to(
                LogoutState::Failed(e.clone()),
                format!("Failed to stop sync service: {}", e),
                0
            ).await?;
            self.handle_error(&e).await;
            return Err(anyhow!(e));
        }
        
        // Server logout
        self.transition_to(
            LogoutState::LoggingOutFromServer,
            "Logging out from server...".to_string(),
            30
        ).await?;
        
        match self.perform_server_logout().await {
            Ok(_) => {
                self.point_of_no_return.store(true, Ordering::Release);
                LOGOUT_POINT_OF_NO_RETURN.store(true, Ordering::Release);
                self.transition_to(
                    LogoutState::PointOfNoReturn,
                    "Point of no return reached".to_string(),
                    50
                ).await?;
                
                // Delete latest user ID after successful logout
                if let Err(e) = delete_latest_user_id().await {
                    log!("Warning: Failed to delete latest user ID: {}", e);
                }
            }
            Err(e) => {
                // Check if it's an M_UNKNOWN_TOKEN error
                if matches!(&e, LogoutError::Recoverable(RecoverableError::ServerLogoutFailed(msg)) if msg.contains("M_UNKNOWN_TOKEN")) {
                    log!("Token already invalidated, continuing with logout");
                    self.point_of_no_return.store(true, Ordering::Release);
                    LOGOUT_POINT_OF_NO_RETURN.store(true, Ordering::Release);
                    self.transition_to(
                        LogoutState::PointOfNoReturn,
                        "Token already invalidated".to_string(),
                        50
                    ).await?;
                    
                    // Delete latest user ID
                    if let Err(e) = delete_latest_user_id().await {
                        log!("Warning: Failed to delete latest user ID: {}", e);
                    }
                } else {
                    // Restart sync service since we haven't reached point of no return
                    if let Some(sync_service) = get_sync_service() {
                        sync_service.start().await;
                    }
                    
                    self.transition_to(
                        LogoutState::Failed(e.clone()),
                        format!("Server logout failed: {}", e),
                        0
                    ).await?;
                    self.handle_error(&e).await;
                    return Err(anyhow!(e));
                }
            }
        }
        
        // From here on, all failures are unrecoverable
        
        // Close tabs (desktop only)
        if self.config.is_desktop {
            self.transition_to(
                LogoutState::ClosingTabs,
                "Closing all tabs...".to_string(),
                60
            ).await?;
            
            if let Err(e) = self.close_all_tabs().await {
                let error = LogoutError::Unrecoverable(UnrecoverableError::PostPointOfNoReturnFailure(e.to_string()));
                self.transition_to(
                    LogoutState::Failed(error.clone()),
                    "Failed to close tabs".to_string(),
                    0
                ).await?;
                self.handle_error(&error).await;
                return Err(anyhow!(error));
            }
        }
        
        // Clean app state
        self.transition_to(
            LogoutState::CleaningAppState,
            "Cleaning up application state...".to_string(),
            70
        ).await?;
        
        if let Err(e) = self.clean_app_state().await {
            let error = LogoutError::Unrecoverable(UnrecoverableError::PostPointOfNoReturnFailure(e.to_string()));
            self.transition_to(
                LogoutState::Failed(error.clone()),
                "Failed to clean app state".to_string(),
                0
            ).await?;
            self.handle_error(&error).await;
            return Err(anyhow!(error));
        }
        
        // Shutdown tasks
        self.transition_to(
            LogoutState::ShuttingDownTasks,
            "Shutting down background tasks...".to_string(),
            80
        ).await?;
        
        self.shutdown_background_tasks().await;
        
        // Restart runtime
        self.transition_to(
            LogoutState::RestartingRuntime,
            "Restarting Matrix runtime...".to_string(),
            90
        ).await?;
        
        if let Err(e) = self.restart_runtime().await {
            let error = LogoutError::Unrecoverable(UnrecoverableError::RuntimeRestartFailed);
            self.transition_to(
                LogoutState::Failed(error.clone()),
                format!("Failed to restart runtime: {}", e),
                0
            ).await?;
            self.handle_error(&error).await;
            return Err(anyhow!(error));
        }
        
        // Success!
        self.transition_to(
            LogoutState::Completed,
            "Logout completed successfully".to_string(),
            100
        ).await?;
        
        // Reset logout in progress flag
        LOGOUT_IN_PROGRESS.store(false, Ordering::Relaxed);
        
        Cx::post_action(LogoutAction::LogoutSuccess);
        Ok(())
    }
    
    // Individual step implementations
    async fn perform_prechecks(&self) -> Result<(), LogoutError> {
        log!("perform_prechecks started");
        
        // Check client existence
        if get_client().is_none() {
            log!("perform_prechecks: client missing");
            return Err(LogoutError::Unrecoverable(UnrecoverableError::ClientMissing));
        }
        
        // Check sync service
        if get_sync_service().is_none() {
            log!("perform_prechecks: sync service missing");
            return Err(LogoutError::Unrecoverable(UnrecoverableError::SyncServiceMissing));
        }
        log!("perform_prechecks: sync service exists");
        
        // Check access token
        if let Some(client) = get_client() {
            if client.access_token().is_none() {
                log!("perform_prechecks: no access token");
                return Err(LogoutError::Recoverable(RecoverableError::NoAccessToken));
            }
            log!("perform_prechecks: access token exists");
        }
        
        log!("perform_prechecks completed successfully");
        Ok(())
    }
    
    async fn stop_sync_service(&self) -> Result<(), LogoutError> {
        if let Some(sync_service) = get_sync_service() {
            sync_service.stop().await;
            Ok(())
        } else {
            Err(LogoutError::Unrecoverable(UnrecoverableError::SyncServiceMissing))
        }
    }
    
    async fn perform_server_logout(&self) -> Result<(), LogoutError> {
        let Some(client) = get_client() else {
            return Err(LogoutError::Unrecoverable(UnrecoverableError::ClientMissing));
        };
        
        match tokio::time::timeout(
            self.config.server_logout_timeout,
            client.matrix_auth().logout()
        ).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(LogoutError::Recoverable(RecoverableError::ServerLogoutFailed(e.to_string()))),
            Err(_) => Err(LogoutError::Recoverable(RecoverableError::Timeout("Server logout timed out".to_string()))),
        }
    }
    
    async fn close_all_tabs(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel::<bool>();
        Cx::post_action(MainDesktopUiAction::CloseAllTabs { on_close_all: tx });
        
        match tokio::time::timeout(self.config.tab_close_timeout, rx).await {
            Ok(Ok(_)) => {
                log!("Received signal that all tabs were closed successfully");
                Ok(())
            }
            Ok(Err(e)) => Err(anyhow!("Failed to close all tabs: {}", e)),
            Err(_) => Err(anyhow!("Timed out waiting for tabs to close")),
        }
    }
    
    async fn clean_app_state(&self) -> Result<()> {
        // Clear resources normally, allowing them to be properly dropped
        // This prevents memory leaks when users logout and login again without closing the app
        CLIENT.lock().unwrap().take();
        log!("Client cleared during logout");
        
        SYNC_SERVICE.lock().unwrap().take();
        log!("Sync service cleared during logout");
        
        REQUEST_SENDER.lock().unwrap().take();
        log!("Request sender cleared during logout");
        
        // Only clear collections that don't contain Matrix SDK objects
        TOMBSTONED_ROOMS.lock().unwrap().clear();
        IGNORED_USERS.lock().unwrap().clear();
        ALL_JOINED_ROOMS.lock().unwrap().clear();
        
        let (tx, rx) = oneshot::channel::<bool>();
        Cx::post_action(LogoutAction::CleanAppState { on_clean_appstate: tx });
        
        match tokio::time::timeout(self.config.app_state_cleanup_timeout, rx).await {
            Ok(Ok(_)) => {
                log!("Received signal that app state was cleaned successfully");
                Ok(())
            }
            Ok(Err(e)) => Err(anyhow!("Failed to clean app state: {}", e)),
            Err(_) => Err(anyhow!("Timed out waiting for app state cleanup")),
        }
    }
    
    async fn shutdown_background_tasks(&self) {
        shutdown_background_tasks().await;
    }
    
    async fn restart_runtime(&self) -> Result<()> {
        start_matrix_tokio()
            .map_err(|e| anyhow!("Failed to restart runtime: {}", e))
    }
    
    /// Handle errors by posting appropriate actions
    async fn handle_error(&self, error: &LogoutError) {
        // Reset logout in progress flag on error (unless we've reached point of no return)
        if !LOGOUT_POINT_OF_NO_RETURN.load(Ordering::Acquire) {
            LOGOUT_IN_PROGRESS.store(false, Ordering::Relaxed);
        }
        
        match error {
            LogoutError::Unrecoverable(UnrecoverableError::ClientMissing) => {
                Cx::post_action(LogoutAction::ApplicationRequiresRestart { 
                    missing_component: MissingComponentType::ClientMissing 
                });
            }
            LogoutError::Unrecoverable(UnrecoverableError::SyncServiceMissing) => {
                Cx::post_action(LogoutAction::ApplicationRequiresRestart { 
                    missing_component: MissingComponentType::SyncServiceMissing 
                });
            }
            LogoutError::Recoverable(RecoverableError::Cancelled) => {
                log!("Logout cancelled by user");
                // Don't post failure action for cancellation
            }
            _ => {
                Cx::post_action(LogoutAction::LogoutFailure(error.to_string()));
            }
        }
    }
}

/// Telemetry data for logout operations
#[derive(Debug, Clone)]
pub struct LogoutTelemetry {
    pub total_duration: Duration,
    pub step_durations: Vec<(String, Duration)>,
    pub final_state: LogoutState,
    pub error_count: u32,
}

impl LogoutStateMachine {
    /// Get telemetry data for the logout operation
    pub async fn get_telemetry(&self) -> LogoutTelemetry {
        let progress = self.progress.lock().await;
        let current_state = self.current_state.lock().await;
        
        LogoutTelemetry {
            total_duration: progress.started_at.elapsed(),
            step_durations: vec![], // Would be populated during execution
            final_state: current_state.clone(),
            error_count: 0, // Would be tracked during execution
        }
    }
}

/// Execute logout using the state machine
pub async fn logout_with_state_machine(is_desktop: bool) -> Result<()> {
    log!("logout_with_state_machine called with is_desktop={}", is_desktop);
    
    let config = LogoutConfig {
        is_desktop,
        ..Default::default()
    };
    
    let state_machine = LogoutStateMachine::new(config);
    let result = state_machine.execute().await;
    
    log!("logout_with_state_machine finished with result: {:?}", result.is_ok());
    result
}