# Importing the necessary libraries and modules
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import numpy as np
import util

BAR_WIDTH= 0.75
BAR_DIST = 0.35
BASE = 1.7
GROUP_DIST = 10

TIMEOUTS = ["50ms","500ms","5000ms"]

fig, ax = plt.subplots()

paxos_1min = [287.7, 2047.2, 18501.4]
paxos_2min = [383.7, 2046.8, 20002]
paxos_4min = [724,	1975,	20506]
paxos_1min_ci = [55,	180,	1505]
paxos_2min_ci = [109,	181,	1884]
paxos_4min_ci = [178,	188,	1847]


raft_1min = [364, 1824.133333, 19502.92308]
raft_2min = [379,	1937,	19803]
raft_4min = [966,	2517,	17607]
raft_1min_ci = [132,492,5416]
raft_2min_ci = [176,529,9970]
raft_4min_ci = [524,702,4313]

raft_pvqc_1min = [227.75, 1548.2, 15407.8]
raft_pvqc_2min = [445,	1665,	13809]
raft_pvqc_4min = [1663,	1929,	15410]
raft_pvqc_1min_ci = [116,190,1664]
raft_pvqc_2min_ci = [229,385,1038]
raft_pvqc_4min_ci = [1050,429,1884]

vr_1min = [60000,60000,60000]
vr_2min = [119970,	119499,	119749]
vr_4min = [239972,	239749,	239500]
vr_1min_ci = [2,2,1]
vr_2min_ci = [3,1,1]
vr_4min_ci = [3,1,1]

mp_1min=[60000,60000,60000]
mp_2min=[120000,119998,	120000]
mp_4min=[240004,240002, 240002]
mp_1min_ci=[5,2,2 ]
mp_2min_ci=[2,2,3]
mp_4min_ci=[5,5,4]

all_1min = [("Omni-Paxos", paxos_1min, paxos_1min_ci), ("Raft", raft_1min, raft_1min_ci), ("Raft PV+CQ", raft_pvqc_1min, raft_pvqc_1min_ci), ("VR", vr_1min, vr_1min_ci), ("Multi-Paxos", mp_1min, mp_1min_ci)]
all_2min = [("Omni-Paxos", paxos_2min, paxos_2min_ci), ("Raft", raft_2min, raft_2min_ci), ("Raft PV+CQ", raft_pvqc_2min, raft_pvqc_2min_ci), ("VR", vr_2min, vr_2min_ci), ("Multi-Paxos", mp_2min, mp_2min_ci)]
all_4min = [("Omni-Paxos", paxos_4min, paxos_4min_ci), ("Raft", raft_4min, raft_4min_ci), ("Raft PV+CQ", raft_pvqc_4min, raft_pvqc_4min_ci), ("VR", vr_4min, vr_4min_ci), ("Multi-Paxos", mp_4min, mp_4min_ci)]

all_groups = [all_1min, all_2min, all_4min]

for k in range(len(all_groups)):
	group_base = (k+1) * GROUP_DIST
	group = all_groups[k]
	for i in range(len(group)):
		(algo, algo_group, ci) = group[i]
		base = group_base + (i+1)*BASE
		algo_group_len = len(algo_group)
		for j in range(algo_group_len):
			timeout = TIMEOUTS[algo_group_len - j-1]
			label = "{} {}".format(algo, timeout)
			bar_dist = (algo_group_len - j)*BAR_DIST
			y = algo_group[algo_group_len - j - 1]/1000
			#bar_dist = (j)*BAR_DIST
			#y = algo_group[j]
			x = base + bar_dist
			#print("x: {}, y: {}".format(x, y))
			error_bar_color = "black"
			if util.colors[label] == "black":
				error_bar_color = "gray"
			plt.bar(x, y, align='edge', width=BAR_WIDTH, color=util.colors[label], alpha=0.95, yerr = ci[algo_group_len - j-1]/1000, ecolor = error_bar_color, error_kw = {"elinewidth": 3.5})

#plt.yscale("log")
SIZE = 20
plt.rc('axes', labelsize=SIZE)    # fontsize of the x and y labels
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=SIZE)    # fontsize of the tick labels


x_ticks = [15.5, 25.5, 35.5]
x_labels = ["1 min", "2 min", "4 min"]
ax.set_xticks(x_ticks)
ax.set_xticklabels(x_labels)
ax.tick_params(axis='both', which='major', labelsize=SIZE)
ax.set_ylabel("Down-time (s)", size=20)
ax.set_xlabel("Partition Duration", size=20)
ax.set_ylim(top=40)
#ax.legend(fontsize=18)
fig = ax.get_figure()
fig.set_size_inches(8.5, 4.5)
fig.savefig("bar_quoromloss_new.pdf", dpi = 600, bbox_inches='tight')