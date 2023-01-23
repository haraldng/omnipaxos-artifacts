# Plotting (To be updated)

We use the `python3` programs found in `visualisation/custom_plotters/bench/atomic_broadcast` for plotting. 

### General Performance
1. Put average execution time found in `results/general/summary/ATOMICBROADCAST.data` and the CIs calculated with `StudentTConfidenceInterval` ([here](https://github.com/anonsub0/kompicsbenches/blob/main/visualisation/custom_plotters/bench/atomic_broadcast/plot_normal.py#L23-L31)).
2. `python3 plot_normal.py`. File will be `normal.pdf`

### Full Partition
1. Put the windowed meta results into a directory and run: `python3 plot_periodic -s <path to directory>`. Will be `periodic.pdf`

### Chained Partition
1. Put the windowed meta results [here](https://github.com/anonsub0/kompicsbenches/blob/main/visualisation/custom_plotters/bench/atomic_broadcast/plot_chained.py#L11-L13).
2. `python3 plot_chained.py`. File will be `chained.pdf`

### Deadlock Partition
1. Put the windowed meta results in the following structure:
```
some_dir 
└───deadlock-1min
│   │ paxos.data
│   │ raft.data
│       
└───deadlock-2min
│   │ paxos.data
│   │ raft.data
│  
└───deadlock-4min
│   │ paxos.data
│   │ raft.data
```
2. `python3 plot_deadlock.py -w 5`. File will be `deadlock.pdf`
### Reconfiguration
1. `python3 plot_tp_windows.py -s <path to windowed meta results> -t <path to target directory> -w 5`

### Leader Election
1. Put the results in a directory with the structure:
```
some_dir 
└───cpu
│   │ omni-500.data
│   │ omni-5000.data
│   │ omni-50000.data
│   │ raft-500.data
│   │ raft-5000.data
│   │ raft-50000.data
│       
└───wan
│   │ omni-500.data
│   │ omni-5000.data
│   │ omni-50000.data
│   │ raft-500.data
│   │ raft-5000.data
│   │ raft-50000.data
```
2. `python3 plot_cpu_wan.py -s <path to some_dir>`. File will be in `cpu_wan.pdf`
