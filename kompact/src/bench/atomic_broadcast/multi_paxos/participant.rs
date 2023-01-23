#[cfg(feature = "measure_io")]
use crate::bench::atomic_broadcast::util::io_metadata::IOMetaData;
use crate::bench::atomic_broadcast::{
    ble::Stop,
    messages::{
        paxos::participant::{MPLeaderSer, *},
        StopMsg as NetStopMsg, StopMsgDeser,
    },
    multi_paxos::{participant::State::Follower, util::LeaderElectionPort},
};
use hashbrown::HashSet;
use kompact::prelude::*;
use rand::{prelude::Rng, thread_rng};
use std::time::Duration;

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L49
#[derive(Debug)]
pub struct ElectionOptions {
    pub(crate) ping_period: Duration,
    pub(crate) timeout_min: Duration,
    pub(crate) timeout_max: Duration,
}

impl ElectionOptions {
    fn get_random_timeout_period(&self) -> Duration {
        let mut rng = thread_rng();
        let min = self.timeout_min.as_millis();
        let max = self.timeout_max.as_millis();
        let rnd: u128 = rng.gen_range(min, max);
        Duration::from_millis(rnd as u64)
    }
}

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L79-L85
#[derive(Copy, Clone, Debug)]
enum State {
    Leader,
    Follower,
}

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala
#[derive(ComponentDefinition)]
pub struct Participant {
    ctx: ComponentContext<Self>,
    /*** Variables directly equivalent to the ones with the same name in the original `Participant` implementation ***/
    callback: ProvidedPort<LeaderElectionPort>,
    index: u64,
    pub(crate) other_participants: Vec<ActorPath>,
    round: u64,
    leader_index: u64,
    ping_timer: Option<ScheduledTimer>,
    no_ping_timer: Option<ScheduledTimer>,
    state: State,
    election_options: ElectionOptions,
    /*** Experiment-specific variables ***/
    stopped: bool,
    stopped_peers: HashSet<u64>,
    stop_ask: Option<Ask<u64, ()>>,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
}

impl Participant {
    pub fn with(
        peers: Vec<ActorPath>,
        pid: u64,
        initial_leader_index: u64,
        election_options: ElectionOptions,
    ) -> Participant {
        let index = pid;
        let state = if index == initial_leader_index {
            State::Leader
        } else {
            State::Follower
        };
        Participant {
            ctx: ComponentContext::uninitialised(),
            callback: ProvidedPort::uninitialised(),
            index,
            other_participants: peers,
            round: 0,
            leader_index: initial_leader_index,
            ping_timer: None,
            no_ping_timer: None,
            state,
            election_options,
            stopped: false,
            stopped_peers: HashSet::new(),
            stop_ask: None,
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

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L109
    fn start_ping_timer(&mut self) {
        let ping_period = self.election_options.ping_period;
        let ping_timer =
            self.schedule_periodic(Duration::from_secs(0), ping_period, move |c, _| {
                c.ping(c.round, c.index);
                Handled::Ok
            });
        self.ping_timer = Some(ping_timer);
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L140
    fn ping(&self, round: u64, leader_index: u64) {
        for peer in &self.other_participants {
            peer.tell_serialised(
                Ping {
                    round,
                    leader_index,
                },
                self,
            )
            .expect("Ping should serialise!");
        }
    }

    fn stop_ping_timer(&mut self) {
        if let Some(ping_timer) = std::mem::take(&mut self.ping_timer) {
            self.cancel_timer(ping_timer);
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L118
    fn start_no_ping_timer(&mut self) {
        let random_period = self.election_options.get_random_timeout_period();
        let no_ping_timer = self.schedule_once(random_period, move |c, _| {
            /*
            info!(
                c.ctx.log(),
                "No ping timeout for leader: {}. Random period: {:?}",
                c.leader_index,
                random_period
            );
            */
            c.round = c.round + 1;
            c.leader_index = c.index;
            c.change_state(State::Leader);
            Handled::Ok
        });
        self.no_ping_timer = Some(no_ping_timer);
    }

    fn stop_no_ping_timer(&mut self) {
        if let Some(no_ping_timer) = std::mem::take(&mut self.no_ping_timer) {
            self.cancel_timer(no_ping_timer);
        }
    }

    fn reset_no_ping_timer(&mut self) {
        self.stop_no_ping_timer();
        self.start_no_ping_timer();
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L149
    fn change_state(&mut self, new_state: State) {
        match (self.state, new_state) {
            (State::Leader, State::Leader) => {}     // Do nothing.
            (State::Follower, State::Follower) => {} // Do nothing.
            (State::Follower, State::Leader) => {
                self.stop_no_ping_timer();
                self.start_ping_timer();
                self.state = State::Leader;
                self.ping(self.round, self.leader_index);
                self.callback.trigger(self.leader_index);
            }
            (State::Leader, State::Follower) => {
                self.stop_ping_timer();
                self.start_no_ping_timer();
                self.state = Follower;
                self.callback.trigger(self.leader_index);
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L182
    fn handle_ping(&mut self, ping: Ping) {
        let ping_ballot = (ping.round, ping.leader_index);
        let ballot = (self.round, self.leader_index);
        match self.state {
            State::Follower => {
                if ping_ballot < ballot {
                    debug!(
                        self.ctx().log(),
                        "A participant received a stale Ping message in round {} with leader {:?} but is already in round {} with leader {}. The ping is being ignored",
                        ping.round, ping.leader_index, self.round, self.leader_index
                    );
                } else if ping_ballot == ballot {
                    self.reset_no_ping_timer();
                } else {
                    self.round = ping.round;
                    self.leader_index = ping.leader_index;
                    self.reset_no_ping_timer();
                }
            }
            State::Leader => {
                if ping_ballot <= ballot {
                    debug!(
                        self.ctx().log(),
                        "A participant received a stale Ping message in round {} with leader {:?} but is already in round {} with leader {}. The ping is being ignored",
                        ping.round, ping.leader_index, self.round, self.leader_index
                    );
                } else {
                    self.round = ping.round;
                    self.leader_index = ping.leader_index;
                    self.change_state(State::Follower);
                }
            }
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn disconnect_peers(&mut self, peers: Vec<u64>, delay: bool, lagging_peer: Option<u64>) {
        // info!(self.ctx.log(), "SIMULATE PARTITION: {:?}, lagging: {:?}", peers, lagging_peer);
        if let Some(lp) = lagging_peer {
            // disconnect from lagging peer first
            self.disconnected_peers.push(lp);
            let a = peers.clone();
            let lagging_delay = self.lagging_delay.unwrap();
            self.schedule_once(lagging_delay, move |c, _| {
                for pid in a {
                    c.disconnected_peers.push(pid);
                }
                Handled::Ok
            });
        } else {
            if delay {
                let lagging_delay = self.lagging_delay.unwrap();
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

impl ComponentLifecycle for Participant {
    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L130-L137
    /// The check to set the intial state is done in the constructor, see `Participant::with()`
    fn on_start(&mut self) -> Handled {
        match self.state {
            State::Leader => self.start_ping_timer(),
            State::Follower => self.start_no_ping_timer(),
        }
        Handled::Ok
    }

    fn on_kill(&mut self) -> Handled {
        self.stop_ping_timer();
        self.stop_no_ping_timer();
        Handled::Ok
    }
}

impl Actor for Participant {
    type Message = Stop;

    fn receive_local(&mut self, stop: Stop) -> Handled {
        #[cfg(feature = "simulate_partition")]
        {
            self.disconnected_peers.clear();
        }
        let pid = stop.0.request();
        self.stop_ping_timer();
        self.stop_no_ping_timer();
        for peer in &self.other_participants {
            peer.tell_serialised(NetStopMsg::Peer(*pid), self)
                .expect("NetStopMsg should serialise!");
        }
        self.stopped = true;
        if self.stopped_peers.len() == self.other_participants.len() {
            stop.0.reply(()).expect("Failed to reply to stop ask!");
        } else {
            self.stop_ask = Some(stop.0);
        }
        Handled::Ok
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L168
    fn receive_network(&mut self, m: NetMessage) -> Handled {
        let NetMessage { data, .. } = m;
        match_deser! {data {
            msg(ping): Ping [using MPLeaderSer] => {
                #[cfg(feature = "simulate_partition")]
                if self.disconnected_peers.contains(&ping.leader_index) {
                    return Handled::Ok;
                }
                self.handle_ping(ping);
            },
            msg(stop): NetStopMsg [using StopMsgDeser] => {
                if let NetStopMsg::Peer(pid) = stop {
                    assert!(self.stopped_peers.insert(pid), "Participant got duplicate stop from peer {}", pid);
                    debug!(self.ctx.log(), "Participant got stopped from peer {}", pid);
                    if self.stopped && self.stopped_peers.len() == self.other_participants.len() {
                        debug!(self.ctx.log(), "Participant got stopped from all peers");
                        self.stop_ask
                            .take()
                            .expect("No stop ask!")
                            .reply(())
                            .expect("Failed to reply ask");
                    }
                }

            },
            err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
            default(_) => unimplemented!("Should be either HeartbeatMsg or NetStopMsg!"),
        }
        }
        Handled::Ok
    }
}

impl Provide<LeaderElectionPort> for Participant {
    fn handle(&mut self, _event: <LeaderElectionPort as Port>::Request) -> Handled {
        unimplemented!()
    }
}
