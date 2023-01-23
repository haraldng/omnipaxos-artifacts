use super::serializers::ProxyLeaderInboundSer;
use crate::bench::atomic_broadcast::multi_paxos::messages::{
    AcceptorInbound, Chosen, Phase2a, Phase2b, ProxyLeaderInbound, ReplicaInbound,
};
use hashbrown::HashMap;
use kompact::prelude::*;
use std::time::Duration;

type AcceptorIndex = u64;

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/ProxyLeader.scala#L87
#[derive(Eq, Hash, PartialEq)]
struct SlotRound {
    slot: u64,
    round: u64,
}

struct Pending {
    phase2a: Phase2a,
    phase2bs: HashMap<AcceptorIndex, Phase2b>,
}

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/ProxyLeader.scala#L90-L99
enum State {
    Pending(Pending),
    Done,
}

#[derive(ComponentDefinition)]
pub struct ProxyLeader {
    ctx: ComponentContext<Self>,
    acceptors: HashMap<u64, ActorPath>,
    replicas: HashMap<u64, ActorPath>,
    states: HashMap<SlotRound, State>,
    majority: usize,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
}

impl ProxyLeader {
    pub(crate) fn with(
        acceptors: HashMap<u64, ActorPath>,
        replicas: HashMap<u64, ActorPath>,
    ) -> Self {
        let majority = acceptors.len() / 2 + 1;
        Self {
            ctx: ComponentContext::uninitialised(),
            acceptors,
            replicas,
            states: HashMap::new(),
            majority,
            #[cfg(feature = "simulate_partition")]
            disconnected_peers: vec![],
            #[cfg(feature = "simulate_partition")]
            lagging_delay: None,
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn set_lagging_delay(&mut self, d: Duration) {
        self.lagging_delay = Some(d);
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/ProxyLeader.scala#L175
    fn handle_phase2a(&mut self, phase2a: Phase2a) {
        let slotround = SlotRound {
            slot: phase2a.slot,
            round: phase2a.round,
        };
        match self.states.get_mut(&slotround) {
            Some(_) => {
                debug!(
                    self.ctx.log(),
                    "A ProxyLeader received a Phase2a in slot {:?} and round {:?}, but it already received this Phase2a. The ProxyLeader is ignoring the message.",
                    phase2a.slot,
                    phase2a.round
                )
            }
            None => {
                for (pid, acceptor) in &self.acceptors {
                    #[cfg(feature = "simulate_partition")]
                    {
                        if self.disconnected_peers.contains(pid) {
                            continue;
                        }
                    }
                    acceptor
                        .tell_serialised(AcceptorInbound::Phase2a(phase2a.clone()), self)
                        .expect("Failed to send message");
                }

                // Update our state.
                self.states.insert(
                    slotround,
                    State::Pending(Pending {
                        phase2a,
                        phase2bs: HashMap::new(),
                    }),
                );
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/ProxyLeader.scala#L217
    fn handle_phase2b(&mut self, phase2b: Phase2b) {
        let slotround = SlotRound {
            slot: phase2b.slot,
            round: phase2b.round,
        };
        match self.states.get_mut(&slotround) {
            // needed to introduce this due to Rust's ownership rules
            None => {
                error!(
                    self.ctx.log(),
                    "A ProxyLeader received a Phase2b in slot {:?} and round {:?}, but it never sent a Phase2a in this round.",
                    phase2b.slot,
                    phase2b.round
                );
            }
            Some(State::Done) => {
                debug!(
                    self.ctx.log(),
                    "A ProxyLeader received a Phase2b in slot {:?} and round {:?}, but it has already chosen a value in this slot and round. The Phase2b is ignored.",
                    phase2b.slot,
                    phase2b.round
                );
            }
            Some(State::Pending(pending)) => {
                let phase2bs = &mut pending.phase2bs;
                let phase2b_slot = phase2b.slot;
                phase2bs.insert(phase2b.acceptor_index, phase2b);
                if phase2bs.len() < self.majority {
                    return;
                }
                let chosen = Chosen {
                    slot: phase2b_slot,
                    command_batch_or_noop: pending.phase2a.command_batch_or_noop.clone(),
                };
                // Let the replicas know that the value has been chosen.
                for (pid, replica) in &self.replicas {
                    #[cfg(feature = "simulate_partition")]
                    {
                        if self.disconnected_peers.contains(pid) {
                            continue;
                        }
                    }
                    replica
                        .tell_serialised(ReplicaInbound::Chosen(chosen.clone()), self)
                        .expect("Failed to send message");
                }
                self.states.insert(slotround, State::Done);
            }
        };
    }

    #[cfg(feature = "simulate_partition")]
    pub fn disconnect_peers(&mut self, peers: Vec<u64>, delay: bool, lagging_peer: Option<u64>) {
        if let Some(lp) = lagging_peer {
            // disconnect from lagging peer first
            self.disconnected_peers.push(lp);
            let a = peers.clone();
            let lagging_delay = self.lagging_delay.expect("No lagging delay");
            self.schedule_once(lagging_delay, move |c, _| {
                for pid in a {
                    c.disconnected_peers.push(pid);
                }
                Handled::Ok
            });
        } else {
            if delay {
                let lagging_delay = self.lagging_delay.expect("No lagging delay");
                self.schedule_once(lagging_delay, move |c, _| {
                    c.disconnected_peers = peers;
                    Handled::Ok
                });
            } else {
                self.disconnected_peers = peers;
            }
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn recover_peers(&mut self) {
        self.disconnected_peers.clear();
    }
}

impl Actor for ProxyLeader {
    type Message = ProxyLeaderInbound;

    /// A leader's ProxyLeader is always the local one
    fn receive_local(&mut self, msg: Self::Message) -> Handled {
        match msg {
            ProxyLeaderInbound::Phase2a(p2a) => self.handle_phase2a(p2a),
            _ => unimplemented!("Local message should only be Phase2a"),
        }
        Handled::Ok
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/ProxyLeader.scala#L153
    fn receive_network(&mut self, msg: NetMessage) -> Handled {
        let NetMessage { data, .. } = msg;
        match_deser! {data {
            msg(l): ProxyLeaderInbound [using ProxyLeaderInboundSer] => {
                match l {
                    ProxyLeaderInbound::Phase2b(p2b) => self.handle_phase2b(p2b),
                    _ => unimplemented!("Remote message should only be Phase2b")
                }
            },
            err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
            default(_) => unimplemented!("Should be ProxyLeaderInbound message!")
        }
        }
        Handled::Ok
    }
}

ignore_lifecycle!(ProxyLeader);
