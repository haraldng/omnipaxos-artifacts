Artifact Evaluation for Omni-Paxos [EuroSys'23]
===========================================

This repository contains instructions and scripts for reproducing the evaluation featured in our Eurosys'23 paper "Omni-Paxos: Breaking the Barriers of Partial Connectivity".

# Project Structure
```bash
├── kompact   # source code for the replicated state machines implemented with Omni-Paxos and other protocols
├── proto     # protobuf messages and structures for controlling distributed execution and defining benchmarks
├── runner    # code for driving benchmark execution and writing results to files.
├── shared_rust # common interface for defining benchmarks written in Rust.
├── shared_scala # general benchmark framework code
└── visualisation # tools for plotting results generated by the benchmarks
```

# Dependencies
We used the following environment for our evaluation:

**Software Dependencies**
- Ubuntu 18.04
- Rust: rustc 1.53.0-nightly (3709ae324 2021-04-25)
- JDK: openjdk 11.0.11 2021-04-20
- Scala 2.13.0
- SBT 1.6.2
- Ammonite 2.3.8
- protobuf 3.6 (in particular protoc)

To install these, run the installation script: `./install_dependencies.sh`

After installation, you may need to update the environment variable for Cargo:
```
source "$HOME/.cargo/env"
```

**Hardware Dependencies**

Our evaluation was performed in Google Cloud Compute using nine `e2-standard-8` VM instances with 8 vCPUs and 32 GB memory.

# Installation Guide
1. Clone this repository: ``git clone https://github.com/haraldng/omnipaxos-artifacts.git``
2. Run the installation script: `./install_dependencies.sh`
3. Build with ``./build.sc``

## Minimal Working Example
To verify that the installation was successful, we can run a minimal benchmark: 
```
./bench.sc fakeRemote --withClients 3 --remoteDir remote_logs --testing true --runName minimal-test
```
This runs a small experiment of 10 runs with Omni-Paxos in steady state with 3 servers for 10s.

### Parameters and Output
The ``bench.sc`` command will be used to run all the experiments. It has the following parameters:
- ``fakeRemote``: the experiment environment. In this case we use the ``fakeRemote`` mode which mimics a distributed setting by starting multiple processes on the local host. The `--withClients` argument is specific for this mode and determines the number of processes it should start. The `--remoteDir` is also optional and specific to `fakeRemote`, it provides the directory for which the processes will be spawned. For deployments in real distributed settings (e.g., as in [Setup Guide](#setup-guide)), we will use `remote` which does not require these arguments see ([Experiments and Expected Results](#experiments-and-expected-results)). 
- `--testing` is an optional argument to run this minimal benchmark. This flag is turned off by default.
- `--runName` is an optional argument to provide a more human-friendly name to the experiment. This name will be used in the path of the output directories such as logs and results.

The output of an experiment is structured as follows:
- Logs will be found in `logs/<run_name>`. The `experiment_runner.out` is produced by the benchmark framework with general logging on the status of connections to the server instances, the number of experiment runs remaining, etc. The `kompact_mixed.out` includes the logging from the experiments, and provides logs the status of the current run, e.g., who is the current leader, the number of decided proposals, how long was the last run, etc. The `kompact_mixed.out` should log is useful for monitoring and ensuring the benchmarks are running as expected.

- Results will be in `meta_results/<run_name>` (not to be interchanged with `results` which is auto-generated by the benchmarking framework but can only produce the duration of the experiment). The `meta_results` include various measurements including the longest down time, total and windowed number of decided proposal.

When an experiment is completed, the logs and meta_results can be zipped into one file using the script:
```
./zip_logs_results.sh <run_name>
```

# Reproducing the EuroSys'23 results
This section describes how to setup and run the benchmarks in order to reproduce the claims made in our EuroSys'23 paper.

## Setup Guide
We performed the evaluation on Google Cloud Compute using nine `e2-standard-8` instances (if you don't have cloud access please see the section [Local Experiments](#local-experiments)). This section describes the steps to recreate the distributed environment from scratch.

1. In Google Cloud, navigate to the "VM instances" console under "Compute Engine" and create an `e2-standard-8` instance named `benchmark-master` in the region `us-central1` with:
   1. OS: Ubuntu 18.04
   2. Boot disk: 32GB
2. SSH into the created instance and clone this repository: ``git clone https://github.com/haraldng/omnipaxos-artifacts.git``
3. Run the installation script: `./install_dependencies.sh`
4. Validate installation by building: `./build.sc`

In the benchmark execution, the master will SSH into the other VM instances (we will refer them as the "server instances"). So we will now create a key-pair **without password** and add the public key among the authorized keys:
5. Run `ssh-keygen` with default settings and no password.
6. Add the generated public key as an authorized key: `cat id_rsa.pub >> /home/<username>/.ssh/authorized_keys`

To avoid having to repeat steps 1-6 for all server instances, we will create a machine image in the Google Cloud console.
7. In the "VM instances" console, press on the `benchmark-master` instance and choose "Create Machine Image".
8. Once the machine image has been created successfully, new instances can be created from it in the "Machine Images" console.

We will need different number of client instances depending on the experiment (see section [Experiments and Expected Results](#experiments-and-expected-results)). Each client instance created from the machine image will have the same state as the master up to step 4. 

9. To make sure that the master can connect to the server instances, manually SSH to them from the master using the internal IP addresses (found in Google Cloud console).
10. Build on the server instance: `./build.sc`

**Configuration**
11. We will now configure the master so it can connect to the client instances via SSH automatically when running the benchmarks. There are two configuration files `master.conf` and `nodes.conf` that need to be set **on the master instance**. When specifying IP addresses of the instances, use the **internal** Google IP addresses (can be seen in the Google cloud console):

#### `master.conf`
Example:
```
10.128.0.4  
username | /home/username/.ssh/id_rsa 
```
First line is the IP address of the master. Second line: the username of servers instances that will be used for SSH, and the path to the private key that we generated in step 5. The values must be separated by `|`.

#### `nodes.conf`
Example when having 5 client instances:
```
10.128.0.5 | /home/username/kompicsbenches | 1
10.128.0.6 | /home/username/kompicsbenches | 2
10.128.0.7 | /home/username/kompicsbenches | 3
10.128.0.8 | /home/username/kompicsbenches | 4
10.128.0.9 | /home/username/kompicsbenches | 5
```

Each line contains the IP address, the path to this repository at the server VM, and the server id assigned to that server. The server ids must be unique. As we describe in experiments E3 and E4 (see section [Experiments and Expected Results](#experiments-and-expected-results)), setting the ids correctly is important in the WAN setting!

## Major Claims
- **C1: Omni-Paxos is the only protocol that can recover from all the described partial connectivity scenarios** (constrained, quorum loss, and chained scenario). The other protocols either deadlock or/and livelock in at least one scenario. This is proven by the experiment (E1) described in §7.2 whose results are illustrated in Figure 7 of the paper.
- **C2: The regular performance of Omni-Paxos is on par with existing state-of-the-art protocols in both LAN and WAN settings**. This is proven by the experiments (E2-E4) described in §7.1 whose results illustrated in Figure 8 of the paper.
- **C3: Omni-Paxos has a lower reconfiguration overhead than Raft.**  When a single server is replaced, there is a drop in throughput of at most 20\% lasting up to 15s for Omni-Paxos, compared to 90\% and 55s for Raft. When replacing a majority of servers, Omni-Paxos records at most 80\% lower throughput but recovers to regular performance after 15s. Raft is completely down for up to 40s and takes up to 120s to recover. This is proven by the experiments (E5) described in §7.3 whose results are illustrated in Figure 9 of the paper.

## Experiments and Expected Results
We use five experiments to evaluate the claims (C1-C3). Under the "Results" header of each experiment description, we specify the output location of the results. This path should be used for plotting the corresponding Figures in `visualisation/Plotting.ipynb`.

Each experiment will require changing to a different branch and rebuilding on the master instance. This can be done by:
```
git checkout <branch_name> ; ./build.sc
```

## Partial Connectivity [E1]
- Approximate duration: 20 human minutes + 98 compute hours
- Branch: `pc`
- Number of instances required: 6. master + 5 client instances (all in the same region).
- Run on master instance: `./bench.sc remote --runName pc`
- Results directory (on master instance): `meta_results/pc`
- How the results support the targeted claim: The plotted figure should show that Omni-Paxos, Raft, and Raft PV+QC can recover from the quorum-loss scenario, while VR and Multi-Paxos are deadlocked. In the constrained scenario Omni-Paxos and Multi-Paxos can recover while the other protocols are deadlocked. In the chained scenario, Multi-Paxos have lower number of decided proposals for all timeouts and scenarios. These results should thus indicate that only Omni-Paxos can tolerate all the scenarios as claimed in (C1).

## Normal LAN [E2]
- Approximate duration: 20 human minutes + 22 compute hours
- Branch: ``normal-lan``
- Number of instances required: 6. master + 5 client instances (all in the same region)
- Run on master instance: `./bench.sc remote --runName normal-lan`
- Results directory (on master instance): `meta_results/normal-lan`
- How the results support the targeted claim:  The plots, should support the claim (C2) by showing that Omni-Paxos, Raft, and Multi-Paxos have similar throughput with overlapping CI intervals.
## Normal 3 servers WAN [E3]
- Approximate duration: 20 human minutes + 11 compute hours
- Branch: `normal-wan3`
- Number of instances required: 4. master + 3 client instances (master and server 3 in us-central1, server 2 in eu-west1, and server 1 in asia-northeast1)
- Run on master instance: `./bench.sc remote --runName normal-wan3`
- Results directory (on master instance): `meta_results/normal-wan3`
- How the results support the targeted claim: Same as E2.
## Normal 5 servers WAN [E4]
- Approximate duration: 20 human minutes + 11 compute hours
- Branch: `normal-wan5`
- Number of instances required: 6. master + 5 client instances (master and server 5 in us-central1, server 3 and 4 in eu-west1, and server 1 and 2 in asia-northeast1)
- Run on master instance: `./bench.sc remote --runName normal-wan5`
- Results directory (on master instance): `meta_results/normal-wan5`
- How the results support the targeted claim: Same as E2.
## Reconfiguration [E5]
- Approximate duration: 20 human minutes + 11 compute hours
- Branch: `reconfig`
- Number of instances required: 9. master + 8 client instances (all in the same region)
- Run on master instance: `./bench.sc remote --runName reconfig`
- Results directory (on master instance): `meta_results/reconfig`
- How the results support the targeted claim: Each experiment folder will have a sub-folder `windowed` with the recorded number of decided proposals for every 5s window. When plotted, we see the throughput over time. The plots should show that Omni-Paxos has a smaller drop over a shorter period of time similar to the values specified in (C3). This should support the claim (C3) that Omni-Paxos has lower reconfiguration overhead with smaller impact on throughput and faster recovery compared to Raft.

## Local Experiments
If you don't have access to GCC or any other cloud provider, it is possible to reproduce claim C1 and C2 on a local machine. 

0. Follow the [Installation Guide](#installation-guide).
1. Switch to the `local` branch and build: ```git checkout local ; ./build.sc```
2. Run experiments E1, E2 with: `./bench.sc fakeRemote --withClients 5 --remoteDir remote_logs --runName local`
3. The results are found in  `meta_results/local`. See last step of E1 and E2 in [Experiments and Expected Results](#experiments-and-expected-results) on how to plot and verify the results.

# Plotting
Please see the jupyter notebook with code and instructions for plotting in ``visualisation/Plotter.ipynb``. Each experiment will be plotted by providing their corresponding results path.

# Miscellaneous
We present a set of practical tips and tools that can be useful for evaluators [here](https://github.com/haraldng/omnipaxos-artifacts/blob/main/MISC.md).

