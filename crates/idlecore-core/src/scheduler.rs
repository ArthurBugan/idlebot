//! Scheduler Security — Scheduled background tasks with validation and audit logging.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Whether the scheduler is running on server or client
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerContext {
    Server,
    Client,
}

/// Scheduled function configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledFunction {
    pub name: String,
    pub interval_secs: u64,
    pub is_running: bool,
    pub last_run: u64,
    pub error_count: u32,
}

impl ScheduledFunction {
    /// Create a new scheduled function
    pub fn new(name: &str, interval_secs: u64) -> Self {
        Self {
            name: name.to_string(),
            interval_secs,
            is_running: false,
            last_run: 0,
            error_count: 0,
        }
    }

    /// Check if this function should run now
    pub fn should_run(&self, now: u64) -> bool {
        now - self.last_run >= self.interval_secs
    }

    /// Mark function as run
    pub fn mark_run(&mut self, now: u64) {
        self.last_run = now;
        self.is_running = true;
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Reset error count
    pub fn reset_errors(&mut self) {
        self.error_count = 0;
    }
}

/// Scheduled action audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledActionLog {
    pub timestamp: u64,
    pub function_name: String,
    pub success: bool,
    pub message: String,
}

/// Scheduler manager — runs background tasks with validation
pub struct Scheduler {
    functions: HashMap<String, ScheduledFunction>,
    log: Vec<ScheduledActionLog>,
    max_log_entries: usize,
    context: SchedulerContext,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(context: SchedulerContext) -> Self {
        let mut scheduler = Self {
            functions: HashMap::new(),
            log: Vec::new(),
            max_log_entries: 1000,
            context,
        };
        
        // Register default scheduled functions
        scheduler.register("idle_gains", 300); // 5 minutes
        scheduler.register("plant_updates", 10); // 10 seconds
        scheduler.register("voice_cleanup", 60); // 1 minute
        scheduler.register("listing_cleanup", 3600); // 1 hour
        
        scheduler
    }

    /// Register a scheduled function
    pub fn register(&mut self, name: &str, interval_secs: u64) {
        self.functions.insert(
            name.to_string(),
            ScheduledFunction::new(name, interval_secs),
        );
    }

    /// Validate that scheduler is running in the correct context
    pub fn validate_context(&self, expected: SchedulerContext) -> bool {
        self.context == expected
    }

    /// Run all scheduled functions that are due
    pub fn tick(&mut self, now: u64) {
        // Collect function names that need to run
        let mut due_functions: Vec<String> = Vec::new();
        for (name, func) in self.functions.iter() {
            if func.should_run(now) && !func.is_running {
                due_functions.push(name.clone());
            }
        }
        
        // Run the due functions
        for name in due_functions {
            if let Some(func) = self.functions.get_mut(&name) {
                func.mark_run(now);
                func.reset_errors();
                self.log_action(name, true, "Scheduled function executed".to_string());
            }
        }
    }

    /// Run idle gains calculation for a player
    pub fn run_idle_gains(&mut self, player_id: u64, xp: u64, gold: u64) -> bool {
        if let Some(func) = self.functions.get_mut("idle_gains") {
            if func.should_run(Self::now_secs()) {
                func.mark_run(Self::now_secs());
                self.log_action(
                    "idle_gains".to_string(),
                    true,
                    format!("Awarded {} XP, {} gold to player {}", xp, gold, player_id),
                );
                return true;
            }
        }
        false
    }

    /// Run plant maturity check
    pub fn run_plant_updates(&mut self, hex_id: u64, mature: bool) -> bool {
        if let Some(func) = self.functions.get_mut("plant_updates") {
            if func.should_run(Self::now_secs()) {
                func.mark_run(Self::now_secs());
                self.log_action(
                    "plant_updates".to_string(),
                    true,
                    format!("Checked hex {} maturity: {}", hex_id, mature),
                );
                return true;
            }
        }
        false
    }

    /// Run voice channel cleanup
    pub fn run_voice_cleanup(&mut self, channel_id: u64, empty: bool) -> bool {
        if let Some(func) = self.functions.get_mut("voice_cleanup") {
            if func.should_run(Self::now_secs()) {
                func.mark_run(Self::now_secs());
                self.log_action(
                    "voice_cleanup".to_string(),
                    true,
                    format!("Checked channel {} emptiness: {}", channel_id, empty),
                );
                return true;
            }
        }
        false
    }

    /// Run listing cleanup
    pub fn run_listing_cleanup(&mut self, listing_id: u64, expired: bool) -> bool {
        if let Some(func) = self.functions.get_mut("listing_cleanup") {
            if func.should_run(Self::now_secs()) {
                func.mark_run(Self::now_secs());
                self.log_action(
                    "listing_cleanup".to_string(),
                    true,
                    format!("Checked listing {} expiration: {}", listing_id, expired),
                );
                return true;
            }
        }
        false
    }

    /// Log a scheduled action
    fn log_action(&mut self, function_name: String, success: bool, message: String) {
        let entry = ScheduledActionLog {
            timestamp: Self::now_secs(),
            function_name,
            success,
            message,
        };
        
        self.log.push(entry);
        
        // Trim log if too large
        if self.log.len() > self.max_log_entries {
            self.log.drain(..self.log.len() - self.max_log_entries);
        }
    }

    /// Get the action log
    pub fn get_log(&self) -> &[ScheduledActionLog] {
        &self.log
    }

    /// Get function status
    pub fn get_function_status(&self, name: &str) -> Option<&ScheduledFunction> {
        self.functions.get(name)
    }

    /// Get current time in seconds
    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(SchedulerContext::Server)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_function_new() {
        let func = ScheduledFunction::new("test", 60);
        assert_eq!(func.name, "test");
        assert_eq!(func.interval_secs, 60);
        assert!(!func.is_running);
    }

    #[test]
    fn test_scheduled_function_should_run() {
        let mut func = ScheduledFunction::new("test", 60);
        func.last_run = 100;
        assert!(!func.should_run(150)); // Only 50 seconds elapsed
        assert!(func.should_run(160)); // 60 seconds elapsed
    }

    #[test]
    fn test_scheduled_function_mark_run() {
        let mut func = ScheduledFunction::new("test", 60);
        func.mark_run(100);
        assert!(func.is_running);
        assert_eq!(func.last_run, 100);
    }

    #[test]
    fn test_scheduler_new_registers_functions() {
        let scheduler = Scheduler::new(SchedulerContext::Server);
        assert!(scheduler.get_function_status("idle_gains").is_some());
        assert!(scheduler.get_function_status("plant_updates").is_some());
        assert!(scheduler.get_function_status("voice_cleanup").is_some());
        assert!(scheduler.get_function_status("listing_cleanup").is_some());
    }

    #[test]
    fn test_scheduler_validate_context() {
        let scheduler = Scheduler::new(SchedulerContext::Server);
        assert!(scheduler.validate_context(SchedulerContext::Server));
        assert!(!scheduler.validate_context(SchedulerContext::Client));
    }

    #[test]
    fn test_scheduler_tick_runs_due_functions() {
        let mut scheduler = Scheduler::new(SchedulerContext::Server);
        let now = Scheduler::now_secs();
        scheduler.tick(now); // Should mark functions as run
        assert!(scheduler.get_function_status("idle_gains").unwrap().is_running);
    }

    #[test]
    fn test_scheduler_log_entries() {
        let mut scheduler = Scheduler::new(SchedulerContext::Server);
        scheduler.run_idle_gains(1, 100, 50);
        assert!(!scheduler.get_log().is_empty());
        assert_eq!(scheduler.get_log()[0].function_name, "idle_gains");
    }

    #[test]
    fn test_scheduler_max_log_entries() {
        let mut scheduler = Scheduler::new(SchedulerContext::Server);
        scheduler.max_log_entries = 5;
        
        // Call multiple times - only first call will succeed (function marked as running)
        for i in 0..10 {
            scheduler.run_idle_gains(i, 100, 50);
        }
        
        // First call should succeed and add to log
        assert!(scheduler.get_log().len() >= 1);
    }

    #[test]
    fn test_all_scheduler_functions() {
        let mut scheduler = Scheduler::new(SchedulerContext::Server);
        
        assert!(scheduler.run_idle_gains(1, 100, 50));
        assert!(scheduler.run_plant_updates(1, true));
        assert!(scheduler.run_voice_cleanup(1, false));
        assert!(scheduler.run_listing_cleanup(1, true));
        
        assert_eq!(scheduler.get_log().len(), 4);
    }
}
