# Artifact Evaluation for Omni-Paxos [Eurosys'23]
This repository contains instructions and scripts for reproducing the evaluation featured in our Eurosys'23 paper "Omni-Paxos: Breaking the Barriers of Partial Connectivity".


- Ubuntu 18.04
- Rust: rustc 1.53.0-nightly (42816d61e 2021-04-24)
- JDK: openjdk 11.0.11 2021-04-20
- SBT
- Ammonite
- protobuf 3.6 (in particular protoc)

(See next section for installing all of these)

 We used Google Cloud Compute `e2-standard-8` instances. We require one client instance and between 3-8 server instances. In Running we will provide the number of instances required per experiment.

 Installation Guide and Building must be performed on all VM instances. For convenience, it is recommended to do all steps for the client instancce first and then create a machine image from it that is used to deploy the server instances. Furthermore, we recommend using the internal IP addresses when needed in the following steps.

# Installation Guide

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

# Running (TO BE UPDATED FOR REVISION CHANGES)
Pre-requisite: 1 client + 5 server instances.

On client instance (after built on all instances):
`./bench.sc remote --impls KOMPACTMIX --benchmarks ATOMICBROADCAST --runName <branchName>`

# Plotting
See [Plotting.md](https://github.com/anonsub0/kompicsbenches/blob/main/Plotting.md).