# Implementation Details of Multi-Paxos
This document describes some implementation details we had to perform while porting the multipaxos implementation from [frankenpaxos](https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos) to our benchmark repository. The changes are mainly due to the differences in Scala and Rust.

In the source code, almost all functions are commented with a link to its identical version in ``frankenpaxos``. The ones that don't have such a link were required when used with Kompact for starting/stopping timers and sending/receiving messages.

## General
We did not implement the unsafe functions and the use of flexible quorums from frankenpaxos.

### Participant
The function [`handleForceNoPing`](https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/election/basic/Participant.scala#L221) is not implemented as it is used to 
pretend as if no ping was received to [artificially force a leader change](https://github.com/mwhittaker/frankenpaxos/blob/cebb56f766d1ca257a1251d3e8104a08a7f85f8e/shared/src/main/scala/frankenpaxos/election/basic/Election.proto#L17). This is not used in our experiments. 

### Replica
Only the leader replica replies to the client. Thus, we also connect the callback of a leader change from `Participant` to it. 

### Batcher
The Raft and OmniPaxos batches by time. Thus, the ``Batcher`` batches by time rather than [size](https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L30).

### Leader
The way Phase1 state is updated in FrankenPaxos does not work well in Rust with multiple mutable borrows. Therefore, we update the Phase1 state in place instead.

### ProxyLeader
A ``Leader`` always sends its messages to the local `ProxyLeader`.

## Other
- Use Rust's BTreemap instead of Scala's SortedMap for the log. 
- Rounds use ``u64`` instead of Scala's `Int`. Instead of checking for -1, we check if an `Option<u64>` is `None`. 