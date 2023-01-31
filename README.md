Artifact Evaluation for Omni-Paxos [EuroSys'23]
===========================================


This repository contains instructions and scripts for reproducing the evaluation featured in our Eurosys'23 paper "Omni-Paxos: Breaking the Barriers of Partial Connectivity".

# Structure
This repository is a distributed benchmarking framework. A benchmark consists of one master and one or more clients. The master uses SSH to start the necessary scripts at the clients.
- The `runner`, `shared_rust` and `shared_scala` modules contain the general benchmarking framework code. The experiments and their parameters (e.g., experiment duration, number of concurrent proposals, election timeouts) are defined in `runner/Benchmarks.scala`.
- The ``kompact`` module holds the source code for the implementations of the RSM systems using Omni-Paxos and the other replication algorithms. 
- The ``visualisation`` module includes code to automate plotting of the results collected by the framework. For the paper, we used some custom plotters that are found in the submodule `custom_plotters`.
- To build and run the benchmarks, we have the scripts ``build.sc`` and `bench.sc`. Please see TODO on how to use them.
- The configuration files `master.conf` and `nodes.conf` must be set when running in a distributed environment. Please refer to TODO on how to set them.

# Dependencies
Our evaluation was performed in the following environment:

- Ubuntu 18.04
- Rust: rustc 1.53.0-nightly (3709ae324 2021-04-25)
- JDK: openjdk 11.0.11 2021-04-20
- Scala 2.13.0
- SBT 1.6.2
- Ammonite 2.3.8
- protobuf 3.6 (in particular protoc)

To install these, run the installation script: `./install_dependencies.sh`

# Minimal Example
This runs the benchmark in ``fakeRemote`` mode which mimics a distributed setting by starting multiple processes on the host.

``
./bench.sc fakeRemote --withClients 5 --remoteDir remote_logs --testing true
``

# Setup and Installation Guide
We performed the evaluation on Google Cloud Compute using `e2-standard-8` instances. This section describes the steps to recreate the distributed environment.  **Note: we will need up to 9 instances for the evaluation. This might require increasing the number of quotas!**

1. In Google Cloud, navigate to the "VM instances" console under "Compute Engine" and create an `e2-standard-8` instance named `benchmark-master` with:
   1. OS: Ubuntu 18.04
   2. Boot disk: 32GB
2. SSH into the created instance and clone this repo: ``git clone https://github.com/haraldng/omnipaxos-artifacts.git``
3. Run the installation script: `./install_dependencies.sh`
4. Validate installation by building: `./build.sc`

The master will SSH into the client instances. So we will create a key-pair **without password** and add it as authorized keys.
5. Run `ssh-keygen` with default settings and no password.
6. Add the generated public key as an authorized key: `cat id_rsa.pub >> /home/<username>/.ssh/authorized_keys`

To avoid having to repeat steps 1-6 for all client instances, we will create a machine image in the Google Cloud console. 
7. In the "VM instances" console, press on the `benchmark-master` instance and choose "Create Machine Image".
8. Once the machine image has been created successfully, new instances can be created from it in the "Machine Images" console. 

We will need different number of client instances depending on the experiment (see section TODO). Each client instance created from the machine image will have the same state as the master up to step 6.

## Configuration
We will now finish the setup so that the master can connect to the client instances. There are two configuration files `master.conf` and `nodes.conf` that need to be set **on the master instance**. When specifying IP addresses of the instances, use the **internal** Google IP addresses (can be seen in the Google cloud console).

### `master.conf`
Example:
```
10.128.0.4  
username | /home/username/.ssh/id_rsa 
```
First line is the IP address of the master. Second line: the username to SSH into client instances and the path to the private key that we generated in step 5. The values must be separated by `|`.

### `nodes.conf`
Example when having 5 client instances:
```
10.128.0.5 | /home/username/kompicsbenches | 1
10.128.0.6 | /home/username/kompicsbenches | 2
10.128.0.7 | /home/username/kompicsbenches | 3
10.128.0.8 | /home/username/kompicsbenches | 4
10.128.0.9 | /home/username/kompicsbenches | 5
```
Each line contains the IP address, the path to this repository at the client VM, and the server id assigned to that client. The server ids must be unique. **For the WAN experiments: make sure that the client VM closest that is in the same region as the master VM is assigned the highest server id!**

# Running

## Normal and Partial Connectivity (LAN)
- Branch: `normal-pc`
- Number of instances required: master + 5 client instances (all in the same region)

## Reconfiguration (LAN)
- Branch: `reconfig`
- Number of instances required: master + 8 client instances (all in the same region)

## Normal 3 servers (WAN)
- Branch: `normal-3`
- Number of instances required: master + 3 client instances (master and server 3 in us-central1, server 2 in eu-west1, and server 1 in asia-northeast1)

## Normal 5 servers (WAN)
- Branch: `normal-5`
- Number of instances required: master + 5 client instances (master and server 5 in us-central1, server 3 and 4 in eu-west1, and server 1 and 2 in asia-northeast1)

# Plotting
Please see the jupyter notebook with code and instructions for plotting in ``visualisation/custom_plotters/atomic_broadcast/Plotter.ipynb``

