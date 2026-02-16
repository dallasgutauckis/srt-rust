//! SRT Connection Bonding
//!
//! This crate implements multi-path bonding for SRT, including socket groups,
//! broadcast mode, backup mode, load balancing, and packet alignment.

pub mod group;
pub mod broadcast;
pub mod backup;
pub mod alignment;
pub mod balancing;

pub use group::{
    GroupError, GroupMember, GroupStats, GroupType, MemberStats, MemberStatus, SocketGroup,
};
pub use broadcast::{
    BroadcastBonding, BroadcastBondingStats, BroadcastError, BroadcastReceiver,
    BroadcastReceiverStats, BroadcastSendResult, BroadcastSender,
};
pub use backup::{
    BackupBonding, BackupBondingStats, BackupError, BackupRole, FailoverEvent, FailoverReason,
};
pub use alignment::{
    AlignedPacket, AlignmentBuffer, AlignmentError, AlignmentStats, PacketSource, PathStats,
    PathTracker,
};
pub use balancing::{
    BalancingAlgorithm, BalancingError, BalancingSendResult, BalancingStats, LoadBalancer,
    PathCapacity,
};
