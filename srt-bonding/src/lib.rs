//! SRT Connection Bonding
//!
//! This crate implements multi-path bonding for SRT, including socket groups,
//! broadcast mode, backup mode, load balancing, and packet alignment.

pub mod alignment;
pub mod backup;
pub mod balancing;
pub mod broadcast;
pub mod group;

pub use alignment::{
    AlignedPacket, AlignmentBuffer, AlignmentError, AlignmentStats, PacketSource, PathStats,
    PathTracker,
};
pub use backup::{
    BackupBonding, BackupBondingStats, BackupError, BackupRole, FailoverEvent, FailoverReason,
};
pub use balancing::{
    BalancingAlgorithm, BalancingError, BalancingSendResult, BalancingStats, LoadBalancer,
    PathCapacity,
};
pub use broadcast::{
    BroadcastBonding, BroadcastBondingStats, BroadcastError, BroadcastReceiver,
    BroadcastReceiverStats, BroadcastSendResult, BroadcastSender,
};
pub use group::{
    GroupError, GroupMember, GroupStats, GroupType, MemberStats, MemberStatus, SocketGroup,
};
