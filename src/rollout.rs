//! Built-in connector capability matrix and staged rollout.
//!
//! This is product data, not runtime branching. A connector implementation
//! must agree with its row before registration.

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorKey, EffectVerb, SourceClass,
};

const RECONCILE: &[Capability] = &[Capability::Reconcile];
const GMAIL_CAPABILITIES: &[Capability] = &[
    Capability::Reconcile,
    Capability::Effect(EffectVerb::MarkRead),
];
const IMESSAGE_CAPABILITIES: &[Capability] = &[
    Capability::Reconcile,
    Capability::Effect(EffectVerb::SendReadReceipt),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinConnector {
    HackerNews,
    Gmail,
    Slack,
    GoogleChat,
    IMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingestion {
    Poll,
    SocketWithReconciliation,
    PullSubscriptionWithReconciliation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupGate {
    None,
    GoogleOAuthRestrictedScope,
    SlackWorkspaceApproval,
    GoogleChatWorkspaceAccess,
    JailbreakBridgeCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectorPlan {
    pub connector: BuiltinConnector,
    pub descriptor: ConnectorDescriptor,
    pub ingestion: Ingestion,
    pub durable_cursor: &'static str,
    pub recovery: &'static str,
    pub setup_gate: SetupGate,
    pub risk: &'static str,
}

impl BuiltinConnector {
    pub const fn plan(self) -> ConnectorPlan {
        match self {
            Self::HackerNews => ConnectorPlan {
                connector: self,
                descriptor: ConnectorDescriptor {
                    key: ConnectorKey::new("hacker-news"),
                    label: "Hacker News",
                    class: SourceClass::PublicFeed,
                    capabilities: RECONCILE,
                },
                ingestion: Ingestion::Poll,
                durable_cursor: "maxitem plus ranking snapshot",
                recovery: "scan unseen numeric item ids, then refresh top/best/new",
                setup_gate: SetupGate::None,
                risk: "ranking endpoints are bounded and are not durable cursors",
            },
            Self::Gmail => ConnectorPlan {
                connector: self,
                descriptor: ConnectorDescriptor {
                    key: ConnectorKey::new("gmail"),
                    label: "Gmail",
                    class: SourceClass::Mail,
                    capabilities: GMAIL_CAPABILITIES,
                },
                ingestion: Ingestion::PullSubscriptionWithReconciliation,
                durable_cursor: "mailbox historyId",
                recovery: "bounded full resync when historyId expires",
                setup_gate: SetupGate::GoogleOAuthRestrictedScope,
                risk: "testing OAuth refresh tokens expire after seven days",
            },
            Self::Slack => ConnectorPlan {
                connector: self,
                descriptor: ConnectorDescriptor {
                    key: ConnectorKey::new("slack"),
                    label: "Slack",
                    class: SourceClass::WorkspaceChat,
                    capabilities: RECONCILE,
                },
                ingestion: Ingestion::SocketWithReconciliation,
                durable_cursor: "workspace, channel, message timestamp",
                recovery: "conversations.history from each channel watermark",
                setup_gate: SetupGate::SlackWorkspaceApproval,
                risk: "the app only sees conversations it is permitted to join",
            },
            Self::GoogleChat => ConnectorPlan {
                connector: self,
                descriptor: ConnectorDescriptor {
                    key: ConnectorKey::new("google-chat"),
                    label: "Google Chat",
                    class: SourceClass::WorkspaceChat,
                    capabilities: RECONCILE,
                },
                ingestion: Ingestion::PullSubscriptionWithReconciliation,
                durable_cursor: "space event time with overlap",
                recovery: "spaceEvents.list per selected space",
                setup_gate: SetupGate::GoogleChatWorkspaceAccess,
                risk: "subscriptions expire and require Google Cloud control-plane setup",
            },
            Self::IMessage => ConnectorPlan {
                connector: self,
                descriptor: ConnectorDescriptor {
                    key: ConnectorKey::new("imessage-smserver"),
                    label: "iMessage via SMServer",
                    class: SourceClass::PersonalMessaging,
                    capabilities: IMESSAGE_CAPABILITIES,
                },
                ingestion: Ingestion::SocketWithReconciliation,
                durable_cursor: "bridge identity plus message GUID",
                recovery: "SMServer REST backfill after WebSocket reconnect",
                setup_gate: SetupGate::JailbreakBridgeCompatibility,
                risk: "private iOS frameworks and background execution are version-fragile",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutStage {
    SyntheticFoundation,
    Connector(BuiltinConnector),
}

pub const fn rollout() -> [RolloutStage; 6] {
    [
        RolloutStage::SyntheticFoundation,
        RolloutStage::Connector(BuiltinConnector::HackerNews),
        RolloutStage::Connector(BuiltinConnector::Gmail),
        RolloutStage::Connector(BuiltinConnector::Slack),
        RolloutStage::Connector(BuiltinConnector::GoogleChat),
        RolloutStage::Connector(BuiltinConnector::IMessage),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn plugin_identity_is_unique_data() {
        let keys = [
            BuiltinConnector::HackerNews,
            BuiltinConnector::Gmail,
            BuiltinConnector::Slack,
            BuiltinConnector::GoogleChat,
            BuiltinConnector::IMessage,
        ]
        .map(|connector| connector.plan().descriptor.key);
        assert_eq!(keys.into_iter().collect::<HashSet<_>>().len(), keys.len());
    }

    #[test]
    fn risky_bridges_land_after_the_reliable_cursor_foundation() {
        assert_eq!(
            rollout(),
            [
                RolloutStage::SyntheticFoundation,
                RolloutStage::Connector(BuiltinConnector::HackerNews),
                RolloutStage::Connector(BuiltinConnector::Gmail),
                RolloutStage::Connector(BuiltinConnector::Slack),
                RolloutStage::Connector(BuiltinConnector::GoogleChat),
                RolloutStage::Connector(BuiltinConnector::IMessage),
            ]
        );
    }

    #[test]
    fn capability_matrix_exposes_only_real_writebacks() {
        let hn = BuiltinConnector::HackerNews.plan().descriptor;
        let gmail = BuiltinConnector::Gmail.plan().descriptor;
        let imessage = BuiltinConnector::IMessage.plan().descriptor;
        assert!(!hn.supports(EffectVerb::MarkRead));
        assert!(gmail.supports(EffectVerb::MarkRead));
        assert!(!gmail.supports(EffectVerb::SendReadReceipt));
        assert!(imessage.supports(EffectVerb::SendReadReceipt));
    }
}
