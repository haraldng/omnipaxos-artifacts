extern crate raft as tikv_raft;

use crate::bench::atomic_broadcast::{
    benchmark_client::{AtomicBroadcastClient, ClientParams},
    benchmark_master::AtomicBroadcastMaster,
};
use benchmark_suite_shared::{
    kompics_benchmarks::benchmarks::AtomicBroadcastRequest, BenchmarkError, DistributedBenchmark,
};
use kompact::prelude::ActorPath;
use std::str::FromStr;

#[derive(Default)]
pub struct AtomicBroadcast;

impl DistributedBenchmark for AtomicBroadcast {
    type MasterConf = AtomicBroadcastRequest;
    type ClientConf = ClientParams;
    type ClientData = ActorPath;
    type Master = AtomicBroadcastMaster;
    type Client = AtomicBroadcastClient;
    const LABEL: &'static str = "AtomicBroadcast";

    fn new_master() -> Self::Master {
        AtomicBroadcastMaster::new()
    }

    fn msg_to_master_conf(
        msg: Box<dyn (::protobuf::Message)>,
    ) -> Result<Self::MasterConf, BenchmarkError> {
        downcast_msg!(msg; AtomicBroadcastRequest)
    }

    fn new_client() -> Self::Client {
        AtomicBroadcastClient::new()
    }

    fn str_to_client_conf(s: String) -> Result<Self::ClientConf, BenchmarkError> {
        Ok(ClientParams { experiment_str: s })
    }

    fn str_to_client_data(str: String) -> Result<Self::ClientData, BenchmarkError> {
        let res = ActorPath::from_str(&str);
        res.map_err(|e| {
            BenchmarkError::InvalidMessage(format!("Could not read client data: {}", e))
        })
    }

    fn client_conf_to_str(c: Self::ClientConf) -> String {
        c.experiment_str
    }

    fn client_data_to_str(d: Self::ClientData) -> String {
        d.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Done;

/*
#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::bench::atomic_broadcast::paxos::ballot_leader_election::Ballot;
    use leaderpaxos::storage::SequenceTraits;

    #[derive(Debug)]
    struct GetSequence(Ask<(), SequenceResp>);

    impl<S> Into<PaxosCompMsg<S>> for GetSequence
    where
        S: SequenceTraits<Ballot>,
    {
        fn into(self) -> PaxosCompMsg<S> {
            PaxosCompMsg::GetSequence(self.0)
        }
    }

    impl Into<RaftCompMsg> for GetSequence {
        fn into(self) -> RaftCompMsg {
            RaftCompMsg::GetSequence(self.0)
        }
    }

    #[derive(Clone, Debug)]
    pub struct SequenceResp {
        pub node_id: u64,
        pub sequence: Vec<u64>,
    }

    impl SequenceResp {
        pub fn with(node_id: u64, sequence: Vec<u64>) -> SequenceResp {
            SequenceResp { node_id, sequence }
        }
    }

    fn create_nodes(
        n: u64,
        algorithm: &str,
        reconfig_policy: &str,
        last_node_id: u64,
    ) -> (
        Vec<KompactSystem>,
        Vec<ActorPath>,
        Vec<Recipient<GetSequence>>,
    ) {
        let mut systems = Vec::with_capacity(n as usize);
        let mut actor_paths = Vec::with_capacity(n as usize);
        let mut actor_refs = Vec::with_capacity(n as usize);
        let mut conf = KompactConfig::default();
        conf.load_config_file(CONFIG_PATH);
        let bc = BufferConfig::from_config_file(CONFIG_PATH);
        bc.validate();
        for i in 1..=n {
            let system = kompact_benchmarks::kompact_system_provider::global()
                .new_remote_system_with_threads_config(
                    format!("node{}", i),
                    4,
                    conf.clone(),
                    bc.clone(),
                    TCP_NODELAY,
                );
            let (actor_path, actor_ref) = match algorithm {
                "paxos" => {
                    let experiment_configs = get_experiment_configs(last_node_id);
                    let reconfig_policy = match reconfig_policy {
                        "none" => None,
                        "eager" => Some(PaxosTransferPolicy::Eager),
                        "pull" => Some(PaxosTransferPolicy::Pull),
                        unknown => panic!("Got unknown Paxos transfer policy: {}", unknown),
                    };
                    let experiment_params = ExperimentParams::load_from_file(CONFIG_PATH);
                    let (paxos_comp, unique_reg_f) = system.create_and_register(|| {
                        PaxosComp::<MemorySequence, MemoryState>::with(
                            experiment_configs,
                            reconfig_policy.unwrap_or(PaxosTransferPolicy::Pull),
                            experiment_params,
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
                    let r: Recipient<GetSequence> = paxos_comp.actor_ref().recipient();
                    (self_path, r)
                }
                "raft" => {
                    let voters = get_experiment_configs(last_node_id).0;
                    let reconfig_policy = match reconfig_policy {
                        "none" => None,
                        "replace-leader" => Some(RaftReconfigurationPolicy::ReplaceLeader),
                        "replace-follower" => Some(RaftReconfigurationPolicy::ReplaceFollower),
                        unknown => panic!("Got unknown Raft transfer policy: {}", unknown),
                    };
                    /*** Setup RaftComp ***/
                    let (raft_comp, unique_reg_f) =
                        system.create_and_register(|| RaftComp::<Storage>::with(voters));
                    unique_reg_f.wait_expect(REGISTER_TIMEOUT, "RaftComp failed to register!");
                    let self_path = system
                        .register_by_alias(&raft_comp, RAFT_PATH)
                        .wait_expect(REGISTER_TIMEOUT, "Communicator failed to register!");
                    let raft_comp_f = system.start_notify(&raft_comp);
                    raft_comp_f
                        .wait_timeout(REGISTER_TIMEOUT)
                        .expect("RaftComp never started!");

                    let r: Recipient<GetSequence> = raft_comp.actor_ref().recipient();
                    (self_path, r)
                }
                unknown => panic!("Got unknown algorithm: {}", unknown),
            };
            systems.push(system);
            actor_paths.push(actor_path);
            actor_refs.push(actor_ref);
        }
        (systems, actor_paths, actor_refs)
    }

    fn check_quorum(sequence_responses: &[SequenceResp], quorum_size: usize, num_proposals: u64) {
        for i in 1..=num_proposals {
            let nodes: Vec<_> = sequence_responses
                .iter()
                .filter(|sr| sr.sequence.contains(&i))
                .map(|sr| sr.node_id)
                .collect();
            let timed_out_proposal = nodes.len() == 0;
            if !timed_out_proposal {
                assert!(nodes.len() >= quorum_size, "Decided value did NOT have majority quorum! proposal_id: {}, contained: {:?}, quorum: {}", i, nodes, quorum_size);
            }
        }
    }

    fn check_validity(sequence_responses: &[SequenceResp], num_proposals: u64) {
        let invalid_nodes: Vec<_> = sequence_responses
            .iter()
            .map(|sr| (sr.node_id, sr.sequence.iter().max().unwrap_or(&0)))
            .filter(|(_, max)| *max > &num_proposals)
            .collect();
        assert!(
            invalid_nodes.len() < 1,
            "Nodes decided unproposed values. Num_proposals: {}, invalied_nodes: {:?}",
            num_proposals,
            invalid_nodes
        );
    }

    fn check_uniform_agreement(sequence_responses: &[SequenceResp]) {
        let longest_seq = sequence_responses
            .iter()
            .max_by(|sr, other_sr| sr.sequence.len().cmp(&other_sr.sequence.len()))
            .expect("Empty SequenceResp from nodes!");
        for sr in sequence_responses {
            assert!(longest_seq.sequence.starts_with(sr.sequence.as_slice()));
        }
    }

    fn run_experiment(
        algorithm: &str,
        num_nodes: u64,
        num_proposals: u64,
        concurrent_proposals: u64,
        reconfiguration: &str,
        reconfig_policy: &str,
    ) {
        let mut master = AtomicBroadcastMaster::new();
        let mut experiment = AtomicBroadcastRequest::new();
        experiment.algorithm = String::from(algorithm);
        experiment.number_of_nodes = num_nodes;
        experiment.number_of_proposals = num_proposals;
        experiment.concurrent_proposals = concurrent_proposals;
        experiment.reconfiguration = String::from(reconfiguration);
        experiment.reconfig_policy = String::from(reconfig_policy);
        let num_nodes_needed = match reconfiguration {
            "off" => num_nodes,
            "single" => num_nodes + 1,
            _ => unimplemented!(),
        };
        let d = DeploymentMetaData::new(num_nodes_needed as u32);
        let (client_systems, clients, client_refs) = create_nodes(
            num_nodes_needed,
            experiment.get_algorithm(),
            experiment.get_reconfig_policy(),
            num_nodes_needed,
        );
        master
            .setup(experiment, &d)
            .expect("Failed to setup master");
        master.prepare_iteration(clients);
        master.run_iteration();

        let mut futures = vec![];
        for client in client_refs {
            let (kprom, kfuture) = promise::<SequenceResp>();
            let ask = Ask::new(kprom, ());
            client.tell(GetSequence(ask));
            futures.push(kfuture);
        }
        let sequence_responses: Vec<_> = FutureCollection::collect_results::<Vec<_>>(futures);
        let quorum_size = num_nodes as usize / 2 + 1;
        check_quorum(&sequence_responses, quorum_size, num_proposals);
        check_validity(&sequence_responses, num_proposals);
        check_uniform_agreement(&sequence_responses);

        master.cleanup_iteration(true, 0.0);
        for system in client_systems {
            system.shutdown().expect("Failed to shutdown system");
        }
    }

    #[test]
    #[ignore]
    fn paxos_normal_test() {
        let num_nodes = 3;
        let num_proposals = 1000;
        let concurrent_proposals = 200;
        let reconfiguration = "off";
        let reconfig_policy = "none";
        run_experiment(
            "paxos",
            num_nodes,
            num_proposals,
            concurrent_proposals,
            reconfiguration,
            reconfig_policy,
        );
    }

    #[test]
    #[ignore]
    fn paxos_reconfig_test() {
        let num_nodes = 3;
        let num_proposals = 1000;
        let concurrent_proposals = 200;
        let reconfiguration = "single";
        let reconfig_policy = "pull";
        run_experiment(
            "paxos",
            num_nodes,
            num_proposals,
            concurrent_proposals,
            reconfiguration,
            reconfig_policy,
        );
    }

    #[test]
    #[ignore]
    fn raft_normal_test() {
        let num_nodes = 3;
        let num_proposals = 1000;
        let concurrent_proposals = 200;
        let reconfiguration = "off";
        let reconfig_policy = "none";
        run_experiment(
            "raft",
            num_nodes,
            num_proposals,
            concurrent_proposals,
            reconfiguration,
            reconfig_policy,
        );
    }

    #[test]
    #[ignore]
    fn raft_reconfig_follower_test() {
        let num_nodes = 3;
        let num_proposals = 1000;
        let concurrent_proposals = 200;
        let reconfiguration = "single";
        run_experiment(
            "raft",
            num_nodes,
            num_proposals,
            concurrent_proposals,
            reconfiguration,
            "replace-follower",
        );
    }

    #[test]
    #[ignore]
    fn raft_reconfig_leader_test() {
        let num_nodes = 3;
        let num_proposals = 1000;
        let concurrent_proposals = 200;
        let reconfiguration = "single";
        run_experiment(
            "raft",
            num_nodes,
            num_proposals,
            concurrent_proposals,
            reconfiguration,
            "replace-leader",
        );
    }
}
 */
