#[cfg(feature = "measure_io")]
use crate::bench::atomic_broadcast::util::io_metadata::IOMetaData;
use crate::bench::atomic_broadcast::{
    ble::{Ballot, BallotLeaderElection, Stop},
    messages::{
        paxos::{ballot_leader_election::*, vr_leader_election::*},
        StopMsg as NetStopMsg, StopMsgDeser,
    },
};
use hashbrown::HashSet;
use kompact::prelude::*;
use omnipaxos::leader_election::Leader;
use std::time::Duration;

#[derive(ComponentDefinition)]
pub struct VRLeaderElectionComp {
    ctx: ComponentContext<Self>,
    ble_port: ProvidedPort<BallotLeaderElection>,
    pid: u64,
    pub(crate) views: Vec<(u64, ActorPath)>,
    view_number: u32,
    hb_round: u32,
    leader_is_alive: bool,
    viewchange_status: ViewChangeStatus,
    latest_ballot_as_leader: Ballot,
    hb_delay: u64,
    pub majority: usize,
    timer: Option<ScheduledTimer>,
    stopped: bool,
    stopped_peers: HashSet<u64>,
    stop_ask: Option<Ask<u64, ()>>,
    quick_timeout: bool,
    initial_election_factor: u64,
    #[cfg(feature = "measure_io")]
    io_metadata: IOMetaData,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
}

impl VRLeaderElectionComp {
    pub fn with(
        views: Vec<(u64, ActorPath)>,
        pid: u64,
        hb_delay: u64,
        quick_timeout: bool,
        initial_election_factor: u64,
    ) -> VRLeaderElectionComp {
        let n = &views.len();
        VRLeaderElectionComp {
            ctx: ComponentContext::uninitialised(),
            ble_port: ProvidedPort::uninitialised(),
            pid,
            majority: n / 2 + 1, // +1 because peers is exclusive ourselves
            views,
            view_number: 0,
            hb_round: 0,
            leader_is_alive: false,
            viewchange_status: ViewChangeStatus::Elected(Ballot::default()),
            latest_ballot_as_leader: Default::default(),
            hb_delay,
            timer: None,
            stopped: false,
            stopped_peers: HashSet::with_capacity(*n - 1),
            stop_ask: None,
            quick_timeout,
            initial_election_factor,
            #[cfg(feature = "measure_io")]
            io_metadata: IOMetaData::default(),
            #[cfg(feature = "simulate_partition")]
            disconnected_peers: vec![],
            #[cfg(feature = "simulate_partition")]
            lagging_delay: None,
        }
    }

    /*
    /// Sets initial state after creation. Should only be used before being started.
    pub fn set_initial_leader(&mut self, l: Leader<Ballot>) {
        self.viewchange_status = ViewChangeStatus::Elected(l.round);
        self.ble_port.trigger(Leader::with(l.pid, l.round));
    }
    */

    #[cfg(feature = "simulate_partition")]
    pub fn set_lagging_delay(&mut self, d: Duration) {
        self.lagging_delay = Some(d);
    }

    fn new_hb_round(&mut self) {
        let delay = if self.quick_timeout {
            // use short timeout if still no first leader
            self.hb_delay / self.initial_election_factor
        } else {
            self.hb_delay
        };
        self.hb_round += 1;
        let hb_request = HeartbeatRequest::with(self.hb_round);
        #[cfg(feature = "measure_io")]
        {
            self.io_metadata.update_sent(&hb_request);
        }
        let leader_ballot = match self.viewchange_status {
            ViewChangeStatus::Elected(b) => b,
            _ => panic!(
                "Tried to send HBRequest without elected leader: {:?}",
                self.viewchange_status
            ),
        };
        self.leader_is_alive = false;
        let (_, leader_ap) = self
            .views
            .iter()
            .find(|x| x.0 == leader_ballot.pid)
            .unwrap();
        leader_ap
            .tell_serialised(HeartbeatMsg::Request(hb_request), self)
            .expect("HBRequest should serialise!");
        self.start_timer(delay, leader_ballot);
    }

    fn hb_timeout(&mut self, b: Ballot) -> Handled {
        match self.viewchange_status {
            ViewChangeStatus::Elected(l) => {
                if l == b {
                    if self.leader_is_alive {
                        self.new_hb_round();
                    } else {
                        let b = self.get_next_view();
                        self.viewchange_status = ViewChangeStatus::ViewChange(b, 1);
                        for (_, peer) in self.views.iter().filter(|x| x.0 != self.pid) {
                            peer.tell_serialised(VRMsg::StartViewChange(b, self.pid), self)
                                .expect("HBRequest should serialise!");
                        }
                    }
                }
            }
            _ => {}
        }
        Handled::Ok
    }

    fn get_next_view(&mut self) -> Ballot {
        self.view_number += 1;
        let (candidate, _) = self
            .views
            .get(self.view_number as usize % self.views.len())
            .unwrap();
        Ballot::with(self.view_number as u32, *candidate)
    }

    fn start_view_change(&self, b: Ballot) {
        for (_, peer) in self.views.iter().filter(|x| x.0 != self.pid) {
            peer.tell_serialised(VRMsg::StartViewChange(b, self.pid), self)
                .expect("VRMsg should serialise!");
        }
    }

    fn start_timer(&mut self, t: u64, b: Ballot) {
        let timer = self.schedule_once(Duration::from_millis(t), move |c, _| c.hb_timeout(b));
        self.timer = Some(timer);
    }

    fn stop_timer(&mut self) {
        if let Some(timer) = self.timer.take() {
            self.cancel_timer(timer);
        }
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

    #[cfg(feature = "measure_io")]
    pub fn get_io_metadata(&mut self) -> IOMetaData {
        self.io_metadata
    }
}

impl ComponentLifecycle for VRLeaderElectionComp {
    fn on_start(&mut self) -> Handled {
        info!(self.ctx.log(), "Starting with views: {:?}", self.views);
        let bc = BufferConfig::default();
        self.ctx.init_buffers(Some(bc), None);
        self.hb_timeout(Ballot::default())
    }

    fn on_kill(&mut self) -> Handled {
        self.stop_timer();
        Handled::Ok
    }
}

impl Provide<BallotLeaderElection> for VRLeaderElectionComp {
    fn handle(&mut self, _: <BallotLeaderElection as Port>::Request) -> Handled {
        unimplemented!()
    }
}

impl Actor for VRLeaderElectionComp {
    type Message = Stop;

    fn receive_local(&mut self, stop: Stop) -> Handled {
        #[cfg(feature = "simulate_partition")]
        {
            self.disconnected_peers.clear();
        }
        let pid = stop.0.request();
        self.stop_timer();
        for (_, peer) in self.views.iter().filter(|(pid, _)| pid != &self.pid) {
            peer.tell_serialised(NetStopMsg::Peer(*pid), self)
                .expect("NetStopMsg should serialise!");
        }
        self.stopped = true;
        if self.stopped_peers.len() == self.views.len() - 1 {
            stop.0.reply(()).expect("Failed to reply to stop ask!");
        } else {
            self.stop_ask = Some(stop.0);
        }
        Handled::Ok
    }

    fn receive_network(&mut self, m: NetMessage) -> Handled {
        let NetMessage { sender, data, .. } = m;
        match_deser! {data {
            msg(hb): HeartbeatMsg [using BallotLeaderSer] => {
                match hb {
                    HeartbeatMsg::Request(req) if !self.stopped => {
                        #[cfg(feature = "measure_io")] {
                            self.io_metadata.update_received(&req);
                        }
                        /*
                        match self.viewchange_status {
                            ViewChangeStatus::Elected(b) if b.pid == self.pid => {
                                let hb_reply = HeartbeatReply::with(req.round, b, true);
                                #[cfg(feature = "measure_io")] {
                                    self.io_metadata.update_sent(&hb_reply);
                                }
                                sender.tell_serialised(HeartbeatMsg::Reply(hb_reply), self).expect("HBReply should serialise!");
                            },
                            _ => {}
                        }*/
                        let hb_reply = HeartbeatReply::with(req.round, self.latest_ballot_as_leader, true);
                        sender.tell_serialised(HeartbeatMsg::Reply(hb_reply), self).expect("HBReply should serialise!");
                    },
                    HeartbeatMsg::Reply(rep) if !self.stopped => {
                        #[cfg(feature = "simulate_partition")] {
                            if self.disconnected_peers.contains(&rep.ballot.pid) {
                                return Handled::Ok;
                            }
                        }
                        #[cfg(feature = "measure_io")] {
                            self.io_metadata.update_received(&rep);
                        }
                        match self.viewchange_status {
                            ViewChangeStatus::Elected(b) if b == rep.ballot => {
                                self.leader_is_alive = true;
                            },
                            _ => {}
                        }
                    },
                    _ => {},
                }
            },
            msg(vr): VRMsg [using VRDeser] => {
                match vr {
                    VRMsg::StartViewChange(b, from) => {
                        #[cfg(feature = "simulate_partition")] {
                            if self.disconnected_peers.contains(&from) {
                                return Handled::Ok;
                            }
                        }
                        match self.viewchange_status {
                            ViewChangeStatus::Elected(l) if b.n > l.n => {
                                self.view_number = b.n;
                                self.viewchange_status = ViewChangeStatus::ViewChange(b, 1);
                                self.start_view_change(b);
                            },
                            ViewChangeStatus::Candidate(l, _) if b.n > l.n => {
                                self.view_number = b.n;
                                self.viewchange_status = ViewChangeStatus::ViewChange(b, 1);
                                self.start_view_change(b);
                            },
                            ViewChangeStatus::ViewChange(l, counter) => {
                                if b.n > l.n {
                                    self.view_number = b.n;
                                    self.viewchange_status = ViewChangeStatus::ViewChange(b, 1);
                                    self.start_view_change(b);
                                } else if b == l {
                                    let new_counter = counter + 1;
                                    if new_counter == self.majority {
                                        if b.pid == self.pid {
                                            self.viewchange_status = ViewChangeStatus::Candidate(b, 1);
                                        } else {
                                            let (_pid, leader_ap) = self.views.iter().find(|x| x.0 == b.pid).unwrap();
                                            leader_ap.tell_serialised(VRMsg::DoViewChange(b, self.pid), self)
                                                .expect("VRMsg should serialise!");
                                            self.viewchange_status = ViewChangeStatus::Elected(l);
                                            self.quick_timeout = false;
                                            self.stop_timer();
                                            self.new_hb_round();
                                        }
                                    } else {
                                        self.viewchange_status = ViewChangeStatus::ViewChange(b, new_counter);
                                    }
                                }
                            }
                            _ => {} // ignore viewchanges with smaller ballot
                        };
                    },
                    VRMsg::DoViewChange(b, from) => {
                        #[cfg(feature = "simulate_partition")] {
                            if self.disconnected_peers.contains(&from) {
                                return Handled::Ok;
                            }
                        }
                        match self.viewchange_status {
                            ViewChangeStatus::Candidate(l, counter) if l == b => {
                                let new_counter = counter + 1;
                                if new_counter == self.majority {
                                    self.stop_timer();
                                    self.ble_port.trigger(Leader::with(self.pid, l));
                                    self.viewchange_status = ViewChangeStatus::Elected(l);
                                    self.leader_is_alive = true;
                                    self.latest_ballot_as_leader = l;
                                    self.quick_timeout = false;
                                } else {
                                    self.viewchange_status = ViewChangeStatus::Candidate(l, new_counter);
                                }
                            },
                            _ => {
                                warn!(self.ctx.log(), "Ignoring DoViewChange {:?}, status: {:?}", b, self.viewchange_status);
                            }
                        }
                    }
                }
            },
            msg(stop): NetStopMsg [using StopMsgDeser] => {
                if let NetStopMsg::Peer(pid) = stop {
                    assert!(self.stopped_peers.insert(pid), "BLE got duplicate stop from peer {}", pid);
                    debug!(self.ctx.log(), "BLE got stopped from peer {}", pid);
                    if self.stopped && self.stopped_peers.len() == self.views.len() - 1 {
                        debug!(self.ctx.log(), "BLE got stopped from all peers");
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

#[derive(Clone, Copy, Debug)]
enum ViewChangeStatus {
    Elected(Ballot),
    Candidate(Ballot, usize),
    ViewChange(Ballot, usize),
}
