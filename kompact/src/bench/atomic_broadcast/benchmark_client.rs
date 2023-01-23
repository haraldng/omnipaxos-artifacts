extern crate raft as tikv_raft;

use super::{
    super::*,
    paxos::PaxosComp,
    raft::RaftComp,
    storage::paxos::{MemorySequence, MemoryState},
};
use crate::bench::atomic_broadcast::{
    multi_paxos::multipaxos::{MultPaxosCompMsg, MultiPaxosComp},
    paxos::{LeaderElection, PaxosCompMsg},
    raft::RaftCompMsg,
    util::exp_util::*,
};
use kompact::prelude::*;
use std::sync::Arc;
use tikv_raft::storage::MemStorage;

pub struct AtomicBroadcastClient {
    system: Option<KompactSystem>,
    paxos_comp: Option<Arc<Component<PaxosComp<MemorySequence, MemoryState>>>>,
    raft_comp: Option<Arc<Component<RaftComp<MemStorage>>>>,
    multipaxos_comp: Option<Arc<Component<MultiPaxosComp>>>,
}

impl AtomicBroadcastClient {
    pub fn new() -> AtomicBroadcastClient {
        AtomicBroadcastClient {
            system: None,
            paxos_comp: None,
            raft_comp: None,
            multipaxos_comp: None,
        }
    }
}

impl DistributedBenchmarkClient for AtomicBroadcastClient {
    type ClientConf = ClientParams;
    type ClientData = ActorPath;

    fn setup(&mut self, c: Self::ClientConf) -> Self::ClientData {
        println!("Setting up Atomic Broadcast (client)");
        let mut conf = KompactConfig::default();
        conf.load_config_file(CONFIG_PATH);
        let bc = BufferConfig::from_config_file(CONFIG_PATH);
        bc.validate();
        let system = crate::kompact_system_provider::global()
            .new_remote_system_with_threads_config("atomicbroadcast", 8, conf, bc, TCP_NODELAY);
        let (params, meta_subdir) = get_deser_clientparams_and_subdir(&c.experiment_str);
        let experiment_params = ExperimentParams::load_from_file(
            CONFIG_PATH,
            params.run_id.as_str(),
            meta_subdir,
            c.experiment_str,
            params.election_timeout_ms,
        );
        let initial_config: Vec<u64> = (1..=params.num_nodes).collect();
        let named_path = match params.algorithm.as_ref() {
            a if a == "paxos" || a == "vr" || a == "mple" => {
                let leader_election = match a {
                    "vr" => LeaderElection::VR,
                    "mple" => LeaderElection::MultiPaxos,
                    _ => LeaderElection::BLE,
                };
                let (paxos_comp, unique_reg_f) = system.create_and_register(|| {
                    PaxosComp::with(
                        initial_config,
                        params.is_reconfig_exp,
                        experiment_params,
                        leader_election,
                    )
                });
                unique_reg_f.wait_expect(REGISTER_TIMEOUT, "ReplicaComp failed to register!");
                let self_path = system
                    .register_by_alias(&paxos_comp, PAXOS_PATH)
                    .wait_expect(REGISTER_TIMEOUT, "Failed to register alias for ReplicaComp");
                let paxos_comp_f = system.start_notify(&paxos_comp);
                paxos_comp_f
                    .wait_timeout(REGISTER_TIMEOUT)
                    .expect("ReplicaComp never started!");
                self.paxos_comp = Some(paxos_comp);
                self_path
            }
            mp if mp == "multi-paxos" => {
                let (mp_comp, unique_reg_f) = system.create_and_register(|| {
                    MultiPaxosComp::with(initial_config, experiment_params)
                });
                unique_reg_f.wait_expect(REGISTER_TIMEOUT, "ReplicaComp failed to register!");
                let self_path = system
                    .register_by_alias(&mp_comp, MULTIPAXOS_PATH)
                    .wait_expect(
                        REGISTER_TIMEOUT,
                        "Failed to register alias for MultiPaxosComp",
                    );
                let mp_comp_f = system.start_notify(&mp_comp);
                mp_comp_f
                    .wait_timeout(REGISTER_TIMEOUT)
                    .expect("MultiPaxosComp never started!");
                self.multipaxos_comp = Some(mp_comp);
                self_path
            }
            r if r == "raft" || r == "raft_pv_qc" => {
                let pv_qc = r == "raft_pv_qc";
                /*** Setup RaftComp ***/
                let (raft_comp, unique_reg_f) = system.create_and_register(|| {
                    RaftComp::<MemStorage>::with(initial_config, experiment_params, pv_qc)
                });
                unique_reg_f.wait_expect(REGISTER_TIMEOUT, "RaftComp failed to register!");
                let self_path = system
                    .register_by_alias(&raft_comp, RAFT_PATH)
                    .wait_expect(REGISTER_TIMEOUT, "Communicator failed to register!");
                let raft_comp_f = system.start_notify(&raft_comp);
                raft_comp_f
                    .wait_timeout(REGISTER_TIMEOUT)
                    .expect("RaftComp never started!");

                self.raft_comp = Some(raft_comp);
                self_path
            }
            unknown => panic!("Got unknown algorithm: {}", unknown),
        };
        self.system = Some(system);
        println!("Got path for Atomic Broadcast actor: {}", named_path);
        named_path
    }

    fn prepare_iteration(&mut self) -> () {
        println!("Preparing Atomic Broadcast (client)");
    }

    fn cleanup_iteration(&mut self, last_iteration: bool) -> () {
        println!("Cleaning up Atomic Broadcast (client)");
        self.kill_child_components();
        println!("KillAsk complete");
        if last_iteration {
            self.kill_parent_component_and_system();
        }
    }
}

impl AtomicBroadcastClient {
    fn kill_child_components(&mut self) {
        if let Some(paxos) = &self.paxos_comp {
            let kill_comps_f = paxos
                .actor_ref()
                .ask_with(|p| PaxosCompMsg::KillComponents(Ask::new(p, ())));
            kill_comps_f
                .wait_timeout(Duration::from_secs(10))
                .expect("Failed to kill components of PaxosComp");
        }
        if let Some(raft) = &self.raft_comp {
            let kill_comps_f = raft
                .actor_ref()
                .ask_with(|p| RaftCompMsg::KillComponents(Ask::new(p, ())));
            kill_comps_f
                .wait_timeout(Duration::from_secs(10))
                .expect("Failed to kill components of RaftComp");
        }
        if let Some(mp) = &self.multipaxos_comp {
            let kill_comps_f = mp
                .actor_ref()
                .ask_with(|p| MultPaxosCompMsg::KillComponents(Ask::new(p, ())));
            kill_comps_f
                .wait_timeout(Duration::from_secs(10))
                .expect("Failed to kill components of MultiPaxosComp");
        }
    }

    fn kill_parent_component_and_system(&mut self) {
        let system = self.system.take().unwrap();
        if let Some(replica) = self.paxos_comp.take() {
            let kill_replica_f = system.kill_notify(replica);
            kill_replica_f
                .wait_timeout(REGISTER_TIMEOUT)
                .expect("Paxos Replica never died!");
        }
        if let Some(raft_replica) = self.raft_comp.take() {
            let kill_raft_f = system.kill_notify(raft_replica);
            kill_raft_f
                .wait_timeout(REGISTER_TIMEOUT)
                .expect("Raft Replica never died!");
        }
        if let Some(mp_comp) = self.multipaxos_comp.take() {
            let kill_mp_f = system.kill_notify(mp_comp);
            kill_mp_f
                .wait_timeout(REGISTER_TIMEOUT)
                .expect("MultiPaxosComp never died!");
        }
        system
            .shutdown()
            .expect("Kompact didn't shut down properly");
    }
}

#[derive(Debug, Clone)]
pub struct ClientParams {
    pub(crate) experiment_str: String,
}

impl ClientParams {
    pub(crate) fn with(c: ClientParamsDeser, reconfig: &str) -> ClientParams {
        let experiment_str = format!(
            "{},{},{},{},{},{}",
            c.algorithm,
            c.num_nodes,
            c.concurrent_proposals,
            reconfig,
            c.election_timeout_ms,
            c.run_id
        );
        ClientParams { experiment_str }
    }
}

#[derive(Debug, Clone)]
pub struct ClientParamsDeser {
    pub algorithm: String,
    pub num_nodes: u64,
    pub concurrent_proposals: u64,
    pub is_reconfig_exp: bool,
    pub election_timeout_ms: u64,
    pub run_id: String,
}

fn get_deser_clientparams_and_subdir(s: &str) -> (ClientParamsDeser, String) {
    let split: Vec<_> = s.split(',').collect();
    assert_eq!(split.len(), 6, "{:?}", split);
    let algorithm = split[0].to_lowercase();
    let num_nodes = split[1]
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{}' does not represent a node id", split[1]));
    let concurrent_proposals = split[2]
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{}' does not represent concurrent proposals", split[2]));
    let reconfig = split[3].to_lowercase();
    let is_reconfig_exp = match reconfig.as_str() {
        "off" => false,
        r if r == "single" || r == "majority" => true,
        other => panic!("Got unexpected reconfiguration: {}", other),
    };
    let election_timeout_split: Vec<_> = split[4].split("ms").collect();
    let election_timeout_ms = election_timeout_split[0]
        .parse::<u64>()
        .unwrap_or_else(|_| {
            panic!(
                "{}' does not represent election timeout",
                election_timeout_split[0]
            )
        });
    let run_id = split[5].clone();
    let cp = ClientParamsDeser {
        algorithm,
        num_nodes,
        concurrent_proposals,
        is_reconfig_exp,
        election_timeout_ms,
        run_id: run_id.to_string(),
    };
    let subdir = create_metaresults_sub_dir(num_nodes, concurrent_proposals, reconfig.as_str());
    (cp, subdir)
}
