//! Routing policy as data plus a small evaluator.
//!
//! [`PolicyTable::parse`] returns the proof that rule identity, precedence,
//! safety constraints, and the total fallback are sound. Downstream routing
//! therefore returns a decision, never an optional value that must be checked
//! again.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::connector::{EffectVerb, SourceClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    Interrupt,
    NextDigest,
    CompressedDigest,
    ArchiveOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Privacy {
    MetadataOnly,
    RedactedExcerpt,
    FullExcerpt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    Story,
    DirectMessage,
    Mention,
    Security,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderTrust {
    Untrusted,
    Trusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionWindow {
    Open,
    Quiet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub class: SourceClass,
    pub signal: Signal,
    pub urgency: Urgency,
    pub trust: SenderTrust,
    pub window: InterruptionWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSelector {
    Any,
    Class(SourceClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalSelector {
    Any,
    OneOf(&'static [Signal]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustSelector {
    Any,
    TrustedOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowSelector {
    Any,
    OpenOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteRule {
    pub key: &'static str,
    pub source: SourceSelector,
    pub signals: SignalSelector,
    pub trust: TrustSelector,
    pub window: WindowSelector,
    pub minimum_urgency: Urgency,
    pub lane: Lane,
    pub privacy: Privacy,
    pub precedence: i16,
}

impl RouteRule {
    fn matches(self, candidate: Candidate) -> bool {
        let source = match self.source {
            SourceSelector::Any => true,
            SourceSelector::Class(class) => class == candidate.class,
        };
        let signal = match self.signals {
            SignalSelector::Any => true,
            SignalSelector::OneOf(signals) => signals.contains(&candidate.signal),
        };
        let trust = match self.trust {
            TrustSelector::Any => true,
            TrustSelector::TrustedOnly => candidate.trust == SenderTrust::Trusted,
        };
        let window = match self.window {
            WindowSelector::Any => true,
            WindowSelector::OpenOnly => candidate.window == InterruptionWindow::Open,
        };
        source && signal && trust && window && candidate.urgency >= self.minimum_urgency
    }

    fn is_total_fallback(self) -> bool {
        self.source == SourceSelector::Any
            && self.signals == SignalSelector::Any
            && self.trust == TrustSelector::Any
            && self.window == WindowSelector::Any
            && self.minimum_urgency == Urgency::Low
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectRule {
    pub key: &'static str,
    pub class: SourceClass,
    pub verb: EffectVerb,
    pub lanes: &'static [Lane],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteDecision {
    pub rule: &'static str,
    pub lane: Lane,
    pub privacy: Privacy,
}

#[derive(Debug, Clone)]
pub struct PolicyTable {
    routes: Vec<RouteRule>,
    effects: Vec<EffectRule>,
}

impl PolicyTable {
    pub fn parse(
        mut routes: Vec<RouteRule>,
        effects: Vec<EffectRule>,
    ) -> Result<Self, PolicyError> {
        if routes.is_empty() {
            return Err(PolicyError::NoRoutes);
        }
        let mut keys = HashSet::new();
        let mut precedence = HashSet::new();
        for rule in &routes {
            if rule.key.is_empty() || !keys.insert(rule.key) {
                return Err(PolicyError::DuplicateRule(rule.key));
            }
            if !precedence.insert(rule.precedence) {
                return Err(PolicyError::DuplicatePrecedence(rule.precedence));
            }
            if rule.lane == Lane::Interrupt {
                match rule.source {
                    SourceSelector::Class(SourceClass::Mail)
                    | SourceSelector::Class(SourceClass::WorkspaceChat)
                    | SourceSelector::Class(SourceClass::PersonalMessaging) => {}
                    SourceSelector::Any
                    | SourceSelector::Class(SourceClass::PublicFeed) => {
                        return Err(PolicyError::UnsafeInterrupt(rule.key));
                    }
                }
                if rule.window != WindowSelector::OpenOnly {
                    return Err(PolicyError::InterruptDuringQuietHours(rule.key));
                }
            }
            if rule.privacy == Privacy::FullExcerpt
                && (rule.trust != TrustSelector::TrustedOnly
                    || rule.source == SourceSelector::Any)
            {
                return Err(PolicyError::UnsafeFullExcerpt(rule.key));
            }
        }
        let fallback_count = routes
            .iter()
            .filter(|rule| rule.is_total_fallback())
            .count();
        if fallback_count != 1 {
            return Err(PolicyError::FallbackCount(fallback_count));
        }
        let fallback_precedence = routes
            .iter()
            .find(|rule| rule.is_total_fallback())
            .expect("count is one")
            .precedence;
        if routes
            .iter()
            .any(|rule| rule.precedence > fallback_precedence)
        {
            return Err(PolicyError::FallbackNotLast);
        }

        let mut effect_keys = HashSet::new();
        for rule in &effects {
            if rule.key.is_empty() || !effect_keys.insert(rule.key) {
                return Err(PolicyError::DuplicateEffectRule(rule.key));
            }
            if rule.lanes.is_empty() {
                return Err(PolicyError::EffectWithoutLane(rule.key));
            }
            if rule.class == SourceClass::PublicFeed {
                return Err(PolicyError::EffectOnPublicFeed(rule.key));
            }
        }

        routes.sort_by_key(|rule| rule.precedence);
        Ok(Self { routes, effects })
    }

    pub fn route(&self, candidate: Candidate) -> RouteDecision {
        let rule = self
            .routes
            .iter()
            .copied()
            .find(|rule| rule.matches(candidate))
            .expect("parsed policy has one total fallback");
        RouteDecision {
            rule: rule.key,
            lane: rule.lane,
            privacy: rule.privacy,
        }
    }

    pub fn effects_for(
        &self,
        candidate: Candidate,
        decision: RouteDecision,
    ) -> impl Iterator<Item = EffectVerb> + '_ {
        self.effects
            .iter()
            .filter(move |rule| {
                rule.class == candidate.class && rule.lanes.contains(&decision.lane)
            })
            .map(|rule| rule.verb)
    }
}

const DIRECT_OR_SECURITY: &[Signal] = &[Signal::DirectMessage, Signal::Security];
const MENTION_OR_DIRECT: &[Signal] = &[Signal::Mention, Signal::DirectMessage];
const INTERRUPT_LANES: &[Lane] = &[Lane::Interrupt, Lane::NextDigest];

pub fn default_policy() -> PolicyTable {
    PolicyTable::parse(
        vec![
            RouteRule {
                key: "trusted-person-critical",
                source: SourceSelector::Class(SourceClass::PersonalMessaging),
                signals: SignalSelector::OneOf(&[Signal::DirectMessage]),
                trust: TrustSelector::TrustedOnly,
                window: WindowSelector::OpenOnly,
                minimum_urgency: Urgency::Critical,
                lane: Lane::Interrupt,
                privacy: Privacy::RedactedExcerpt,
                precedence: 10,
            },
            RouteRule {
                key: "trusted-mail-critical",
                source: SourceSelector::Class(SourceClass::Mail),
                signals: SignalSelector::OneOf(DIRECT_OR_SECURITY),
                trust: TrustSelector::TrustedOnly,
                window: WindowSelector::OpenOnly,
                minimum_urgency: Urgency::Critical,
                lane: Lane::Interrupt,
                privacy: Privacy::MetadataOnly,
                precedence: 20,
            },
            RouteRule {
                key: "workspace-attention",
                source: SourceSelector::Class(SourceClass::WorkspaceChat),
                signals: SignalSelector::OneOf(MENTION_OR_DIRECT),
                trust: TrustSelector::Any,
                window: WindowSelector::Any,
                minimum_urgency: Urgency::Normal,
                lane: Lane::NextDigest,
                privacy: Privacy::MetadataOnly,
                precedence: 30,
            },
            RouteRule {
                key: "personal-message",
                source: SourceSelector::Class(SourceClass::PersonalMessaging),
                signals: SignalSelector::OneOf(&[Signal::DirectMessage]),
                trust: TrustSelector::Any,
                window: WindowSelector::Any,
                minimum_urgency: Urgency::Normal,
                lane: Lane::NextDigest,
                privacy: Privacy::MetadataOnly,
                precedence: 40,
            },
            RouteRule {
                key: "mail-attention",
                source: SourceSelector::Class(SourceClass::Mail),
                signals: SignalSelector::OneOf(DIRECT_OR_SECURITY),
                trust: TrustSelector::Any,
                window: WindowSelector::Any,
                minimum_urgency: Urgency::Normal,
                lane: Lane::NextDigest,
                privacy: Privacy::MetadataOnly,
                precedence: 50,
            },
            RouteRule {
                key: "public-feed",
                source: SourceSelector::Class(SourceClass::PublicFeed),
                signals: SignalSelector::Any,
                trust: TrustSelector::Any,
                window: WindowSelector::Any,
                minimum_urgency: Urgency::Low,
                lane: Lane::CompressedDigest,
                privacy: Privacy::RedactedExcerpt,
                precedence: 60,
            },
            RouteRule {
                key: "archive-rest",
                source: SourceSelector::Any,
                signals: SignalSelector::Any,
                trust: TrustSelector::Any,
                window: WindowSelector::Any,
                minimum_urgency: Urgency::Low,
                lane: Lane::ArchiveOnly,
                privacy: Privacy::MetadataOnly,
                precedence: 100,
            },
        ],
        vec![
            EffectRule {
                key: "mail-read-after-paper",
                class: SourceClass::Mail,
                verb: EffectVerb::MarkRead,
                lanes: INTERRUPT_LANES,
            },
            EffectRule {
                key: "message-receipt-after-paper",
                class: SourceClass::PersonalMessaging,
                verb: EffectVerb::SendReadReceipt,
                lanes: INTERRUPT_LANES,
            },
        ],
    )
    .expect("built-in policy is a parsed constant")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    NoRoutes,
    DuplicateRule(&'static str),
    DuplicatePrecedence(i16),
    UnsafeInterrupt(&'static str),
    InterruptDuringQuietHours(&'static str),
    UnsafeFullExcerpt(&'static str),
    FallbackCount(usize),
    FallbackNotLast,
    DuplicateEffectRule(&'static str),
    EffectWithoutLane(&'static str),
    EffectOnPublicFeed(&'static str),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoutes => f.write_str("policy has no routes"),
            Self::DuplicateRule(key) => write!(f, "route rule {key:?} is empty or duplicated"),
            Self::DuplicatePrecedence(value) => {
                write!(f, "route precedence {value} is duplicated")
            }
            Self::UnsafeInterrupt(key) => {
                write!(f, "route rule {key:?} may interrupt for a public or unknown source")
            }
            Self::InterruptDuringQuietHours(key) => {
                write!(f, "route rule {key:?} may interrupt during quiet hours")
            }
            Self::UnsafeFullExcerpt(key) => {
                write!(f, "route rule {key:?} may expose an untrusted full excerpt")
            }
            Self::FallbackCount(count) => {
                write!(f, "policy must have exactly one total fallback, found {count}")
            }
            Self::FallbackNotLast => f.write_str("the total fallback is not last"),
            Self::DuplicateEffectRule(key) => {
                write!(f, "effect rule {key:?} is empty or duplicated")
            }
            Self::EffectWithoutLane(key) => write!(f, "effect rule {key:?} has no lanes"),
            Self::EffectOnPublicFeed(key) => {
                write!(f, "effect rule {key:?} targets a public feed")
            }
        }
    }
}

impl Error for PolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(class: SourceClass, signal: Signal) -> Candidate {
        Candidate {
            class,
            signal,
            urgency: Urgency::Normal,
            trust: SenderTrust::Untrusted,
            window: InterruptionWindow::Open,
        }
    }

    #[test]
    fn public_feeds_can_never_enter_the_interrupt_lane() {
        let policy = default_policy();
        for signal in [
            Signal::Story,
            Signal::DirectMessage,
            Signal::Mention,
            Signal::Security,
            Signal::Update,
        ] {
            let mut input = candidate(SourceClass::PublicFeed, signal);
            input.urgency = Urgency::Critical;
            input.trust = SenderTrust::Trusted;
            assert_ne!(policy.route(input).lane, Lane::Interrupt);
        }
    }

    #[test]
    fn quiet_hours_are_a_coordinate_in_the_rule_not_a_post_route_patch() {
        let policy = default_policy();
        let mut input = candidate(SourceClass::PersonalMessaging, Signal::DirectMessage);
        input.urgency = Urgency::Critical;
        input.trust = SenderTrust::Trusted;
        assert_eq!(policy.route(input).lane, Lane::Interrupt);
        input.window = InterruptionWindow::Quiet;
        assert_eq!(policy.route(input).lane, Lane::NextDigest);
    }

    #[test]
    fn successful_paper_delivery_has_source_specific_effect_data() {
        let policy = default_policy();
        let input = candidate(SourceClass::Mail, Signal::DirectMessage);
        let decision = policy.route(input);
        assert_eq!(
            policy.effects_for(input, decision).collect::<Vec<_>>(),
            vec![EffectVerb::MarkRead]
        );
    }

    #[test]
    fn parser_refuses_a_flag_pile_that_can_interrupt_any_source() {
        let rule = RouteRule {
            key: "bad",
            source: SourceSelector::Any,
            signals: SignalSelector::Any,
            trust: TrustSelector::Any,
            window: WindowSelector::OpenOnly,
            minimum_urgency: Urgency::Low,
            lane: Lane::Interrupt,
            privacy: Privacy::MetadataOnly,
            precedence: 1,
        };
        assert!(matches!(
            PolicyTable::parse(vec![rule], Vec::new()),
            Err(PolicyError::UnsafeInterrupt("bad"))
        ));
    }
}
