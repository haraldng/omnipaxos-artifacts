use crate::{
    bench::{
        atomic_broadcast::{
            benchmark::Done,
            communicator::Communicator,
            messages::{
                AtomicBroadcastDeser, AtomicBroadcastMsg, StopMsg as NetStopMsg, StopMsgDeser,
            },
            multi_paxos::{
                acceptor::Acceptor,
                batcher::Batcher,
                leader::{Leader, LeaderOptions},
                participant::{ElectionOptions, Participant},
                proxy_leader::ProxyLeader,
                replica::{Replica, ReplicaOptions},
                util::LeaderElectionPort,
            },
            util::exp_util::ExperimentParams,
        },
        serialiser_ids::*,
    },
    partitioning_actor::{PartitioningActorMsg, PartitioningActorSer},
};

#[cfg(feature = "simulate_partition")]
use crate::bench::atomic_broadcast::messages::{PartitioningExpMsg, PartitioningExpMsgDeser};
#[cfg(feature = "simulate_partition")]
use crate::bench::atomic_broadcast::util::exp_util::LAGGING_DELAY_FACTOR;
use hashbrown::HashMap;
use kompact::prelude::*;
use std::{ops::DerefMut, sync::Arc, time::Duration};

const BATCHER: &str = "batcher";
const PARTICIPANT: &str = "participant";
const LEADER: &str = "leader";
const ACCEPTOR: &str = "acceptor";
const REPLICA: &str = "replica";
const PROXY_LEADER: &str = "proxy_leader";

struct AllActorPaths {
    participant_peers: Vec<ActorPath>,
    leaders: HashMap<u64, ActorPath>,
    acceptors: HashMap<u64, ActorPath>,
    replicas: HashMap<u64, ActorPath>,
    proxy_leaders: HashMap<u64, ActorPath>,
}

#[derive(ComponentDefinition)]
pub struct MultiPaxosComp {
    ctx: ComponentContext<Self>,
    pid: u64,
    initial_nodes: Vec<u64>,
    nodes: Vec<ActorPath>, // derive actorpaths of peers' leader, acceptor, replica, and participant
    communicator_comps: Vec<Arc<Component<Communicator>>>,
    leader_comp: Option<Arc<Component<Leader>>>,
    acceptor_comp: Option<Arc<Component<Acceptor>>>,
    replica_comp: Option<Arc<Component<Replica>>>,
    participant_comp: Option<Arc<Component<Participant>>>,
    proxy_leader_comp: Option<Arc<Component<ProxyLeader>>>,
    batcher_comp: Option<Arc<Component<Batcher>>>,
    iteration_id: u32,
    stopped: bool,
    partitioning_actor: Option<ActorPath>,
    cached_client: Option<ActorPath>,
    current_leader: u64,
    experiment_params: ExperimentParams,
}

impl MultiPaxosComp {
    pub fn with(initial_nodes: Vec<u64>, experiment_params: ExperimentParams) -> Self {
        Self {
            ctx: ComponentContext::uninitialised(),
            pid: 0,
            initial_nodes,
            nodes: vec![],
            communicator_comps: vec![],
            leader_comp: None,
            acceptor_comp: None,
            replica_comp: None,
            participant_comp: None,
            proxy_leader_comp: None,
            batcher_comp: None,
            iteration_id: 0,
            stopped: false,
            partitioning_actor: None,
            cached_client: None,
            current_leader: 0,
            experiment_params,
        }
    }

    fn reset_state(&mut self) {
        self.pid = 0;
        self.nodes.clear();
        self.iteration_id = 0;
        self.stopped = false;
        self.current_leader = 0;
        self.cached_client = None;
        self.partitioning_actor = None;

        self.leader_comp = None;
        self.acceptor_comp = None;
        self.replica_comp = None;
        self.participant_comp = None;
        self.communicator_comps.clear();
    }

    fn create_options(&self) -> (ElectionOptions, LeaderOptions, ReplicaOptions) {
        let config = self.ctx.config();
        // participant params
        let timeout_delta_factor = config["multipaxos"]["timeout_delta_factor"]
            .as_i64()
            .unwrap() as u64;
        let timeout_delta = self.experiment_params.election_timeout_ms / timeout_delta_factor;
        // leader params
        let resend_phase1as_period = config["multipaxos"]["resend_phase1as_period"]
            .as_duration()
            .unwrap();
        let noop_flush_period = config["multipaxos"]["noop_flush_period"]
            .as_duration()
            .unwrap();

        // replica params
        let log_grow_size = config["multipaxos"]["log_grow_size"].as_i64().unwrap() as usize;
        let send_chosen_watermark_every_n_entries = config["multipaxos"]
            ["send_chosen_watermark_every_n_entries"]
            .as_i64()
            .unwrap() as u64;
        let recover_log_entry_factor = config["multipaxos"]["recover_log_entry_factor"]
            .as_i64()
            .unwrap() as u64;
        let recover_log_entry_min_period =
            self.experiment_params.election_timeout_ms * recover_log_entry_factor;
        let recover_log_entry_max_period = recover_log_entry_min_period * 2;
        let ping_period_factor =
            config["multipaxos"]["ping_period_factor"].as_i64().unwrap() as u64;
        let ping_period_ms = self.experiment_params.election_timeout_ms / ping_period_factor;

        let election_opts = ElectionOptions {
            ping_period: Duration::from_millis(ping_period_ms),
            timeout_min: Duration::from_millis(
                self.experiment_params.election_timeout_ms - timeout_delta,
            ),
            timeout_max: Duration::from_millis(
                self.experiment_params.election_timeout_ms + timeout_delta,
            ),
        };
        let leader_opts = LeaderOptions {
            resend_phase1as_period,
            noop_flush_period,
        };
        let replica_opts = ReplicaOptions {
            log_grow_size,
            send_chosen_watermark_every_n_entries,
            recover_log_entry_min_period: Duration::from_millis(recover_log_entry_min_period),
            recover_log_entry_max_period: Duration::from_millis(recover_log_entry_max_period),
        };

        (election_opts, leader_opts, replica_opts)
    }

    fn create_components(&mut self) -> Handled {
        let system = self.ctx.system();
        let client_path = self.cached_client.as_ref().expect("No cached client");
        let AllActorPaths {
            participant_peers,
            leaders,
            acceptors,
            replicas,
            proxy_leaders,
        } = self.derive_actorpaths(&self.initial_nodes);
        let (election_opts, leader_opts, replica_opts) = self.create_options();
        let initial_leader = self.nodes.len() as u64;
        let (participant, participant_f) = system.create_and_register(|| {
            Participant::with(participant_peers, self.pid, initial_leader, election_opts)
        });
        let (proxy_leader, proxy_leader_f) =
            system.create_and_register(|| ProxyLeader::with(acceptors.clone(), replicas));
        let (leader, leader_f) = system.create_and_register(|| {
            Leader::with(
                self.pid,
                leader_opts,
                acceptors,
                proxy_leader.actor_ref(),
                client_path.clone(),
            )
        });
        let (acceptor, acceptor_f) =
            system.create_and_register(|| Acceptor::with(self.pid, leaders.clone(), proxy_leaders));
        let (replica, replica_f) = system.create_and_register(|| {
            Replica::with(
                self.pid,
                replica_opts,
                self.nodes.len() as u64,
                leaders.clone(),
                client_path.clone(),
            )
        });
        let (batcher, batcher_f) = system.create_and_register(|| Batcher::with(self.pid, leaders));

        #[cfg(feature = "simulate_partition")]
        {
            let lagging_delay_ms =
                self.experiment_params.election_timeout_ms / LAGGING_DELAY_FACTOR;
            let lagging_delay = Duration::from_millis(lagging_delay_ms);

            participant.on_definition(|c| c.set_lagging_delay(lagging_delay));
            batcher.on_definition(|c| c.set_lagging_delay(lagging_delay));
            leader.on_definition(|c| c.set_lagging_delay(lagging_delay));
            proxy_leader.on_definition(|c| c.set_lagging_delay(lagging_delay));
            acceptor.on_definition(|c| c.set_lagging_delay(lagging_delay));
            replica.on_definition(|c| c.set_lagging_delay(lagging_delay));
        }
        let participant_alias_f = system.register_by_alias(
            &participant,
            Self::create_alias(PARTICIPANT, self.pid, self.iteration_id),
        );
        let batcher_alias_f = system.register_by_alias(
            &batcher,
            Self::create_alias(BATCHER, self.pid, self.iteration_id),
        );
        let leader_alias_f = system.register_by_alias(
            &leader,
            Self::create_alias(LEADER, self.pid, self.iteration_id),
        );
        let proxy_leader_alias_f = system.register_by_alias(
            &proxy_leader,
            Self::create_alias(PROXY_LEADER, self.pid, self.iteration_id),
        );
        let acceptor_alias_f = system.register_by_alias(
            &acceptor,
            Self::create_alias(ACCEPTOR, self.pid, self.iteration_id),
        );
        let replica_alias_f = system.register_by_alias(
            &replica,
            Self::create_alias(REPLICA, self.pid, self.iteration_id),
        );

        let futures = vec![
            participant_f,
            proxy_leader_f,
            leader_f,
            acceptor_f,
            replica_f,
            batcher_f,
        ];

        let alias_futures = vec![
            participant_alias_f,
            proxy_leader_alias_f,
            leader_alias_f,
            acceptor_alias_f,
            replica_alias_f,
        ];

        biconnect_components::<LeaderElectionPort, _, _>(&participant, &leader)
            .expect("Could not connect Participant and Leader");
        biconnect_components::<LeaderElectionPort, _, _>(&participant, &replica)
            .expect("Could not connect Participant and Replica");

        Handled::block_on(self, move |mut async_self| async move {
            for f in futures {
                f.await
                    .unwrap()
                    .expect("Failed to register when creating components");
            }
            for f in alias_futures {
                f.await.unwrap().expect("Failed to register aliases");
            }
            let batcher_path = batcher_alias_f.await.unwrap().unwrap();
            #[cfg(feature = "simulate_partition")]
            leader.on_definition(|c| c.local_batcher = Some(batcher_path));

            async_self.participant_comp = Some(participant);
            async_self.leader_comp = Some(leader);
            async_self.acceptor_comp = Some(acceptor);
            async_self.replica_comp = Some(replica);
            async_self.proxy_leader_comp = Some(proxy_leader);
            async_self.batcher_comp = Some(batcher);

            async_self
                .partitioning_actor
                .take()
                .expect("No partitioning actor found!")
                .tell_serialised(
                    PartitioningActorMsg::InitAck(async_self.iteration_id),
                    async_self.deref_mut(),
                )
                .expect("Should serialise InitAck");
        })
    }

    fn start_components(&self) {
        let participant = self.participant_comp.as_ref().expect("No participant");
        let batcher = self.batcher_comp.as_ref().expect("No batcher");
        let leader = self.leader_comp.as_ref().expect("No leader");
        let proxy_leader = self.proxy_leader_comp.as_ref().expect("No proxy_leader");
        let acceptor = self.acceptor_comp.as_ref().expect("No acceptor");
        let replica = self.replica_comp.as_ref().expect("No replica");
        self.ctx.system().start(participant);
        self.ctx.system().start(batcher);
        self.ctx.system().start(leader);
        self.ctx.system().start(proxy_leader);
        self.ctx.system().start(acceptor);
        self.ctx.system().start(replica);
    }

    fn kill_components(&mut self, ask: Ask<(), Done>) -> Handled {
        info!(self.ctx.log(), "Kill components");
        let system = self.ctx.system();
        let mut kill_futures = Vec::with_capacity(6);
        if let Some(participant) = self.participant_comp.take() {
            let kill_participant = system.kill_notify(participant);
            kill_futures.push(kill_participant);
        }
        if let Some(batcher) = self.batcher_comp.take() {
            let kill_batcher = system.kill_notify(batcher);
            kill_futures.push(kill_batcher);
        }
        if let Some(proxy_leader) = self.proxy_leader_comp.take() {
            let kill_proxy_leader = system.kill_notify(proxy_leader);
            kill_futures.push(kill_proxy_leader);
        }
        if let Some(leader) = self.leader_comp.take() {
            let kill_leader = system.kill_notify(leader);
            kill_futures.push(kill_leader);
        }
        if let Some(acceptor) = self.acceptor_comp.take() {
            let kill_acceptor = system.kill_notify(acceptor);
            kill_futures.push(kill_acceptor);
        }
        if let Some(replica) = self.replica_comp.take() {
            let kill_replica = system.kill_notify(replica);
            kill_futures.push(kill_replica);
        }
        Handled::block_on(self, move |_| async move {
            for f in kill_futures {
                f.await.expect("Failed to kill");
            }
            let _ = ask.reply(Done);
        })
    }

    fn create_alias(component_name: &str, pid: u64, iteration_id: u32) -> String {
        format!("{}{}-{}", component_name, pid, iteration_id)
    }

    fn get_node_actorpath(&self, pid: u64) -> &ActorPath {
        let idx = pid as usize - 1;
        self.nodes
            .get(idx)
            .unwrap_or_else(|| panic!("Could not get actorpath of pid: {}", pid))
    }

    fn derive_component_actorpath(&self, n: &NamedPath, str: &str, pid: u64) -> ActorPath {
        let sys_path = n.system();
        let protocol = sys_path.protocol();
        let port = sys_path.port();
        let addr = sys_path.address();
        let named_path = NamedPath::new(
            protocol,
            *addr,
            port,
            vec![Self::create_alias(str, pid, self.iteration_id)],
        );
        ActorPath::Named(named_path)
    }

    fn derive_actorpaths(&self, nodes: &[u64]) -> AllActorPaths {
        let num_nodes = nodes.len();
        let mut leaders = HashMap::with_capacity(num_nodes);
        let mut acceptors = HashMap::with_capacity(num_nodes);
        let mut replicas = HashMap::with_capacity(num_nodes);
        let mut proxy_leaders = HashMap::with_capacity(num_nodes);
        let mut participant_peers = Vec::with_capacity(num_nodes);
        for pid in nodes {
            let actorpath = self.get_node_actorpath(*pid);
            match actorpath {
                ActorPath::Named(n) => {
                    let named_leader = self.derive_component_actorpath(n, LEADER, *pid);
                    let named_acceptor = self.derive_component_actorpath(n, ACCEPTOR, *pid);
                    let named_replica = self.derive_component_actorpath(n, REPLICA, *pid);
                    let named_proxy_leader = self.derive_component_actorpath(n, PROXY_LEADER, *pid);

                    leaders.insert(*pid, named_leader);
                    acceptors.insert(*pid, named_acceptor);
                    replicas.insert(*pid, named_replica);
                    proxy_leaders.insert(*pid, named_proxy_leader);
                    if *pid != self.pid {
                        // participant only need peers
                        let named_participant =
                            self.derive_component_actorpath(n, PARTICIPANT, *pid);
                        participant_peers.push(named_participant);
                    }
                }
                _ => error!(
                    self.ctx.log(),
                    "{}",
                    format!("Actorpath is not named for node {}", pid)
                ),
            }
        }
        AllActorPaths {
            participant_peers,
            leaders,
            acceptors,
            replicas,
            proxy_leaders,
        }
    }
}
#[derive(Debug)]
pub enum MultPaxosCompMsg {
    KillComponents(Ask<(), Done>),
}

impl Actor for MultiPaxosComp {
    type Message = MultPaxosCompMsg;

    fn receive_local(&mut self, msg: Self::Message) -> Handled {
        match msg {
            MultPaxosCompMsg::KillComponents(ask) => self.kill_components(ask),
        }
    }

    fn receive_network(&mut self, m: NetMessage) -> Handled {
        match m.data.ser_id {
            ATOMICBCAST_ID => {
                match_deser! {m {
                    msg(am): AtomicBroadcastMsg [using AtomicBroadcastDeser] => {
                        match am {
                            AtomicBroadcastMsg::Proposal(p) => {
                                self.batcher_comp
                                    .as_ref()
                                    .expect("No active Batcher")
                                    .actor_ref()
                                    .tell(p);
                            }
                            AtomicBroadcastMsg::ReconfigurationProposal(_) => {
                                unimplemented!("Reconfiguration in Multi-Paxos")
                            }
                            _ => {}
                        }
                    }
                }}
            }
            #[cfg(feature = "simulate_partition")]
            PARTITIONING_EXP_ID => {
                match_deser! {m {
                    msg(p): PartitioningExpMsg [using PartitioningExpMsgDeser] => {
                        match p {
                            PartitioningExpMsg::DisconnectPeers((peers, delay), lagging_peer) => {
                                // info!(self.ctx.log(), "Disconnecting from peers: {:?}, lagging_peer: {:?}", peers, lagging_peer);
                                self.participant_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers.clone(), delay, lagging_peer.clone()));
                                self.batcher_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers.clone(), delay, lagging_peer.clone()));
                                self.proxy_leader_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers.clone(), delay, lagging_peer.clone()));
                                self.leader_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers.clone(), delay, lagging_peer.clone()));
                                self.acceptor_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers.clone(), delay, lagging_peer.clone()));
                                self.replica_comp.as_ref().unwrap().on_definition(|x| x.disconnect_peers(peers, delay, lagging_peer));
                            }
                            PartitioningExpMsg::RecoverPeers => {
                                //info!(self.ctx.log(), "Reconnecting to peers");
                                self.participant_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                                self.batcher_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                                self.proxy_leader_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                                self.leader_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                                self.acceptor_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                                self.replica_comp.as_ref().unwrap().on_definition(|x| x.recover_peers());
                            }
                        }
                    }
                }}
            }
            _ => {
                let NetMessage { sender, data, .. } = m;
                match_deser! {data {
                    msg(p): PartitioningActorMsg [using PartitioningActorSer] => {
                        match p {
                            PartitioningActorMsg::Init(init) => {
                                self.reset_state();
                                info!(self.ctx.log(), "MultiPaxos got init, pid: {}", init.pid);
                                self.iteration_id = init.init_id;
                                let ser_client = init.init_data.expect("Init should include ClientComp's actorpath");
                                let client = ActorPath::deserialise(&mut ser_client.as_slice()).expect("Failed to deserialise Client's actorpath");
                                self.cached_client = Some(client);
                                self.nodes = init.nodes;
                                self.pid = init.pid as u64;
                                self.partitioning_actor = Some(sender);
                                self.stopped = false;
                                let handled = self.create_components();
                                return handled;
                            },
                            PartitioningActorMsg::Run => {
                                self.start_components();
                                if self.pid == self.nodes.len() as u64 {
                                    self.schedule_once(Duration::from_secs(3), move |c, _| {
                                        let client = c.cached_client.as_ref().unwrap();
                                        client.tell_serialised(AtomicBroadcastMsg::Leader(c.pid, 0), c).expect("Failed to send Leader");
                                        Handled::Ok
                                    });
                                }
                            },
                            _ => {},
                        }
                    },
                    msg(client_stop): NetStopMsg [using StopMsgDeser] => {
                        if let NetStopMsg::Client = client_stop {
                            info!(self.ctx.log(), "Replying to client stop");
                            sender.tell_serialised(NetStopMsg::Peer(self.pid), self).expect("Failed to reply client stop");
                        }
                    },
                    err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
                    default(_) => unimplemented!("Expected either PartitioningActorMsg or NetStopMsg!"),
                }
                }
            }
        }
        Handled::Ok
    }
}

ignore_lifecycle!(MultiPaxosComp);
