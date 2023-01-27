Artifact Evaluation for Omni-Paxos [EuroSys'23]
===========================================


This repository contains instructions and scripts for reproducing the evaluation featured in our Eurosys'23 paper "Omni-Paxos: Breaking the Barriers of Partial Connectivity".

# Structure
This repository is a distributed benchmarking framework. A benchmark consists of one master and one or more clients. The master uses SSH to start the necessary scripts at the clients.
- The `runner`, `shared_rust` and `shared_scala` modules contain the general benchmarking framework code. The experiments and their parameters (e.g., experiment duration, number of concurrent proposals, election timeouts) are defined in `runner/Benchmarks.scala`
- The ``kompact`` module holds the source code for the implementations of the RSM systems using Omni-Paxos and the other replication algorithms. 
- The ``visualisation`` module includes code to automate plotting of the results collected by the framework. For the paper, we used some custom plotters that are found in the submodule `custom_plotters`.
- To build and run the benchmarks, we have the scripts ``build.sc`` and `bench.sc`. Please see TODO on how to use them.
- The configuration files `master.conf` and `nodes.conf` must be set when running in a distributed environment. Please refer to TODO on how to set them.

# Environment
Our evaluation was performed in the following environment TODO
- Ubuntu 18.04
- Rust: rustc 1.53.0-nightly (42816d61e 2021-04-24)
- JDK: openjdk 11.0.11 2021-04-20
- SBT
- Ammonite
- protobuf 3.6 (in particular protoc)

(See next section for installing all of these)

## Installation Guide

1. Clone this repo and run installation script: `./install_dependencies.sh`

On client instance:

2. Create ssh keys (without passcode): `ssh-keygen`

On all server instances:

3. Add the public key created in step 3 to `~/.ssh/authorized_keys`

On client instance:

4. `ssh` to all servers to authenticate and make sure client can reach servers.
5. Add all client ip addresses to the file `nodes.conf`. Also add all ip addresses to the file `kompact/configs/atomic_broadcast.conf` under "deployment"
6. Set `runnerAddr`  to `<client_ip>:45678` and `masterAddr` to `<client_ip>:45679` [here](https://github.com/anonsub0/kompicsbenches/blob/main/bench.sc#L18-L20). Furthermore set ssh key [here](https://github.com/anonsub0/kompicsbenches/blob/main/bench.sc#L326).

# Building
On all instances:

`./build.sc --useOnly shared_scala; ./build.sc --useOnly runner; ./build.sc --useOnly kompact`

# Running

## Minimal Example
To run a minimal example, we can run an experiment for 1 minute on the local (master) instance. 

``
./bench.sc fakeRemote --withClients 5 --remoteDir remote_logs --testing true
``

This runs the benchmark in ``fakeRemote`` mode which mimics a distributed setting by starting multiple processes on the host.

## Distributed Benchmarks

 We used Google Cloud Compute `e2-standard-8` instances. We require one client instance and between 3-8 server instances. In Running we will provide the number of instances required per experiment.

 Installation Guide and Building must be performed on all VM instances. For convenience, it is recommended to do all steps for the client instancce first and then create a machine image from it that is used to deploy the server instances. Furthermore, we recommend using the internal IP addresses when needed in the following steps.

## Configuration
There are two configuration files `master.conf` and `nodes.conf` that need to be set on the master instance.
### `master.conf`
Example:
```
10.128.0.4  
user | /home/user/.ssh/id_rsa 
```
First line is the IP address of the master. Second line: the SSH username to be used for SSH into client instances and the path to the RSA private key used for SSH. The values must be separated by `|`.

### `nodes.conf`
Example when having 5 nodes:
```
10.128.0.5 | /home/user/kompicsbenches | 1
10.128.0.6 | /home/user/kompicsbenches | 2
10.128.0.7 | /home/user/kompicsbenches | 3
10.128.0.8 | /home/user/kompicsbenches | 4
10.128.0.9 | /home/user/kompicsbenches | 5
```
Each line contains the IP address, the path to this repository at the client VM, and the server id assigned to that client. The server ids must be unique. **For the WAN experiments: make sure that the client VM closest that is in the same region as the master VM is assigned the highest server id!**

# Plotting




# Running (TO BE UPDATED FOR REVISION CHANGES)
Pre-requisite: 1 client + 5 server instances.

On client instance (after built on all instances):
`./bench.sc remote --impls KOMPACTMIX --benchmarks ATOMICBROADCAST --runName <branchName>`

# Plotting
See [Plotting.md](https://github.com/anonsub0/kompicsbenches/blob/main/Plotting.md).