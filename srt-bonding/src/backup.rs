//! Backup Bonding Mode
//!
//! Primary/backup link management with automatic failover.
//! Sends on primary, automatically switches to backup on failure.

use crate::group::{GroupError, MemberStatus, SocketGroup};
use parking_lot::RwLock;
use srt_protocol::SeqNumber;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Backup mode errors
#[derive(Error, Debug)]
pub enum BackupError {
    #[error("No primary member configured")]
    NoPrimary,

    #[error("No backup members available")]
    NoBackup,

    #[error("Group error: {0}")]
    Group(#[from] GroupError),

    #[error("All members failed")]
    AllMembersFailed,
}

/// Member role in backup mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRole {
    /// Primary link (actively used)
    Primary,
    /// Backup link (standby)
    Backup,
}

/// Failover event
#[derive(Debug, Clone)]
pub struct FailoverEvent {
    /// Time of failover
    pub timestamp: Instant,
    /// Old primary member ID
    pub old_primary: u32,
    /// New primary member ID
    pub new_primary: u32,
    /// Reason for failover
    pub reason: FailoverReason,
}

/// Reason for failover
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverReason {
    /// Primary link failed
    PrimaryFailed,
    /// Primary link quality degraded
    QualityDegraded,
    /// Manual failover
    Manual,
}

/// Backup bonding manager
pub struct BackupBonding {
    /// Socket group
    group: Arc<SocketGroup>,
    /// Current primary member ID
    primary_id: Arc<RwLock<Option<u32>>>,
    /// Backup member IDs (ordered by priority)
    backup_ids: Arc<RwLock<Vec<u32>>>,
    /// Failover history
    failover_history: Arc<RwLock<Vec<FailoverEvent>>>,
    /// Health check interval
    health_check_interval: Duration,
    /// Last health check time
    last_health_check: Arc<RwLock<Instant>>,
    /// Failure threshold for triggering failover
    failure_threshold: u32,
}

impl BackupBonding {
    /// Create new backup bonding
    pub fn new(
        group: Arc<SocketGroup>,
        health_check_interval: Duration,
        failure_threshold: u32,
    ) -> Self {
        BackupBonding {
            group,
            primary_id: Arc::new(RwLock::new(None)),
            backup_ids: Arc::new(RwLock::new(Vec::new())),
            failover_history: Arc::new(RwLock::new(Vec::new())),
            health_check_interval,
            last_health_check: Arc::new(RwLock::new(Instant::now())),
            failure_threshold,
        }
    }

    /// Set primary member
    pub fn set_primary(&self, member_id: u32) -> Result<(), BackupError> {
        // Verify member exists
        self.group
            .get_member(member_id)
            .ok_or(GroupError::MemberNotFound(member_id))?;

        // Update member status
        if let Some(old_primary) = *self.primary_id.read() {
            self.group
                .update_member_status(old_primary, MemberStatus::Idle)?;
        }

        self.group
            .update_member_status(member_id, MemberStatus::Active)?;
        *self.primary_id.write() = Some(member_id);

        Ok(())
    }

    /// Add backup member
    pub fn add_backup(&self, member_id: u32) -> Result<(), BackupError> {
        // Verify member exists
        self.group
            .get_member(member_id)
            .ok_or(GroupError::MemberNotFound(member_id))?;

        self.group
            .update_member_status(member_id, MemberStatus::Idle)?;

        let mut backups = self.backup_ids.write();
        if !backups.contains(&member_id) {
            backups.push(member_id);
        }

        Ok(())
    }

    /// Get current primary member ID
    pub fn get_primary_id(&self) -> Option<u32> {
        *self.primary_id.read()
    }

    /// Get backup member IDs
    pub fn get_backup_ids(&self) -> Vec<u32> {
        self.backup_ids.read().clone()
    }

    /// Send data on primary link
    pub fn send(&self, data: &[u8]) -> Result<SeqNumber, BackupError> {
        let primary_id = self.get_primary_id().ok_or(BackupError::NoPrimary)?;

        let member = self
            .group
            .get_member(primary_id)
            .ok_or(BackupError::NoPrimary)?;

        match member.connection.send(data) {
            Ok(_) => {
                member.record_sent(data.len());
                Ok(self.group.next_sequence())
            }
            Err(_) => {
                // Primary failed, attempt failover
                self.handle_primary_failure(primary_id, FailoverReason::PrimaryFailed)?;

                // Retry on new primary
                let new_primary_id = self.get_primary_id().ok_or(BackupError::NoPrimary)?;

                let new_member = self
                    .group
                    .get_member(new_primary_id)
                    .ok_or(BackupError::NoPrimary)?;

                new_member
                    .connection
                    .send(data)
                    .map_err(|_| BackupError::AllMembersFailed)?;

                new_member.record_sent(data.len());
                Ok(self.group.next_sequence())
            }
        }
    }

    /// Handle primary link failure
    fn handle_primary_failure(
        &self,
        failed_primary: u32,
        reason: FailoverReason,
    ) -> Result<(), BackupError> {
        // Mark old primary as broken
        self.group
            .update_member_status(failed_primary, MemberStatus::Broken)?;

        // Find next available backup
        let new_primary = {
            let backups = self.backup_ids.read();
            backups
                .iter()
                .find(|&&id| {
                    if let Some(member) = self.group.get_member(id) {
                        member.get_stats().status == MemberStatus::Idle
                    } else {
                        false
                    }
                })
                .copied()
                .ok_or(BackupError::NoBackup)?
            // Drop read lock here
        };

        // Promote backup to primary
        self.set_primary(new_primary)?;

        // Record failover event
        let event = FailoverEvent {
            timestamp: Instant::now(),
            old_primary: failed_primary,
            new_primary,
            reason,
        };

        self.failover_history.write().push(event.clone());

        tracing::info!(
            "Failover: {} -> {} (reason: {:?})",
            failed_primary,
            new_primary,
            reason
        );

        Ok(())
    }

    /// Perform health check on primary
    pub fn health_check(&self) -> Result<bool, BackupError> {
        let now = Instant::now();
        let mut last_check = self.last_health_check.write();

        if now.duration_since(*last_check) < self.health_check_interval {
            return Ok(true); // Too soon for another check
        }

        *last_check = now;

        let primary_id = match self.get_primary_id() {
            Some(id) => id,
            None => return Ok(false),
        };

        let member = match self.group.get_member(primary_id) {
            Some(m) => m,
            None => return Ok(false),
        };

        let stats = member.get_stats();

        // Check for failures
        if stats.failure_count >= self.failure_threshold {
            self.handle_primary_failure(primary_id, FailoverReason::QualityDegraded)?;
            return Ok(false);
        }

        // Check if member is still connected
        if stats.status != MemberStatus::Active {
            self.handle_primary_failure(primary_id, FailoverReason::PrimaryFailed)?;
            return Ok(false);
        }

        Ok(true)
    }

    /// Manual failover to specific backup
    pub fn manual_failover(&self, new_primary_id: u32) -> Result<(), BackupError> {
        let old_primary = self.get_primary_id().ok_or(BackupError::NoPrimary)?;

        // Verify new primary is a backup
        {
            let backups = self.backup_ids.read();
            if !backups.contains(&new_primary_id) {
                return Err(BackupError::Group(GroupError::MemberNotFound(
                    new_primary_id,
                )));
            }
            // Drop read lock here before acquiring write locks
        }

        // Demote old primary to backup
        self.group
            .update_member_status(old_primary, MemberStatus::Idle)?;
        self.backup_ids.write().push(old_primary);

        // Promote new primary
        self.set_primary(new_primary_id)?;
        self.backup_ids.write().retain(|&id| id != new_primary_id);

        // Record event
        let event = FailoverEvent {
            timestamp: Instant::now(),
            old_primary,
            new_primary: new_primary_id,
            reason: FailoverReason::Manual,
        };

        self.failover_history.write().push(event);

        Ok(())
    }

    /// Get failover history
    pub fn failover_history(&self) -> Vec<FailoverEvent> {
        self.failover_history.read().clone()
    }

    /// Get statistics
    pub fn stats(&self) -> BackupBondingStats {
        BackupBondingStats {
            primary_id: self.get_primary_id(),
            backup_ids: self.get_backup_ids(),
            failover_count: self.failover_history.read().len(),
            group_stats: self.group.get_stats(),
        }
    }
}

/// Backup bonding statistics
#[derive(Debug, Clone)]
pub struct BackupBondingStats {
    /// Current primary member ID
    pub primary_id: Option<u32>,
    /// Backup member IDs
    pub backup_ids: Vec<u32>,
    /// Number of failovers that have occurred
    pub failover_count: usize,
    /// Group statistics
    pub group_stats: crate::group::GroupStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::GroupType;
    use srt_protocol::Connection;

    fn create_test_group() -> Arc<SocketGroup> {
        Arc::new(SocketGroup::new(1, GroupType::Backup, 10))
    }

    fn create_test_connection(id: u32) -> Arc<Connection> {
        Arc::new(Connection::new(
            id,
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            SeqNumber::new(1000),
            120,
        ))
    }

    #[test]
    fn test_backup_creation() {
        let group = create_test_group();
        let backup = BackupBonding::new(group, Duration::from_secs(1), 3);

        assert!(backup.get_primary_id().is_none());
        assert!(backup.get_backup_ids().is_empty());
    }

    #[test]
    fn test_set_primary() {
        let group = create_test_group();
        let conn = create_test_connection(1);

        group
            .add_member(conn, "127.0.0.1:9001".parse().unwrap())
            .unwrap();

        let backup = BackupBonding::new(group, Duration::from_secs(1), 3);
        backup.set_primary(1).unwrap();

        assert_eq!(backup.get_primary_id(), Some(1));
    }

    #[test]
    fn test_add_backup() {
        let group = create_test_group();
        let conn1 = create_test_connection(1);
        let conn2 = create_test_connection(2);

        group
            .add_member(conn1, "127.0.0.1:9001".parse().unwrap())
            .unwrap();
        group
            .add_member(conn2, "127.0.0.1:9002".parse().unwrap())
            .unwrap();

        let backup = BackupBonding::new(group, Duration::from_secs(1), 3);
        backup.set_primary(1).unwrap();
        backup.add_backup(2).unwrap();

        assert_eq!(backup.get_primary_id(), Some(1));
        assert_eq!(backup.get_backup_ids(), vec![2]);
    }

    #[test]
    fn test_manual_failover() {
        let group = create_test_group();
        let conn1 = create_test_connection(1);
        let conn2 = create_test_connection(2);

        group
            .add_member(conn1, "127.0.0.1:9001".parse().unwrap())
            .unwrap();
        group
            .add_member(conn2, "127.0.0.1:9002".parse().unwrap())
            .unwrap();

        let backup = BackupBonding::new(group, Duration::from_secs(1), 3);
        backup.set_primary(1).unwrap();
        backup.add_backup(2).unwrap();

        // Manual failover
        backup.manual_failover(2).unwrap();

        assert_eq!(backup.get_primary_id(), Some(2));
        assert_eq!(backup.failover_history().len(), 1);
    }

    #[test]
    fn test_stats() {
        let group = create_test_group();
        let conn = create_test_connection(1);

        group
            .add_member(conn, "127.0.0.1:9001".parse().unwrap())
            .unwrap();

        let backup = BackupBonding::new(group, Duration::from_secs(1), 3);
        backup.set_primary(1).unwrap();

        let stats = backup.stats();
        assert_eq!(stats.primary_id, Some(1));
        assert_eq!(stats.failover_count, 0);
    }
}
