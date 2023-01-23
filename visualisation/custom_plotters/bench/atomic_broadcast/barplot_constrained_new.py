# Importing the necessary libraries and modules
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import numpy as np
import util
from mpl_toolkits.axes_grid1.inset_locator import inset_axes

BAR_WIDTH= 0.75
BAR_DIST = 0.35
BASE = 1.7
GROUP_DIST = 10

TIMEOUTS = ["50ms","500ms","5000ms"]

fig, ax = plt.subplots()

paxos_1min = [302,	1438,	13530]
paxos_2min = [358,	1885,	13501]
paxos_4min = [333,	1697,	13525]
paxos_1min_ci = [96,140,53]
paxos_2min_ci = [99,383,36]
paxos_4min_ci = [110,217,81]

raft_1min = [59993,	59946,	59701]
raft_2min = [119992,119944,	119100]
raft_4min = [239993,239973,	239181]
raft_1min_ci = [2,32,146]
raft_2min_ci = [6,20,495]
raft_4min_ci = [4,68,641]

raft_pvqc_1min = [59993,	59946,	59701]
raft_pvqc_2min = [119992,119944,	119100]
raft_pvqc_4min = [239993,239973,	239181]
raft_pvqc_1min_ci = [2,32,146]
raft_pvqc_2min_ci = [6,20,495]
raft_pvqc_4min_ci = [4,68,641]	

vr_1min = [60796,	60049,	60062]
vr_2min = [121551,	120362,	120073]
vr_4min = [240828,	241014,	240220]
vr_1min_ci = [291,167,26]
vr_2min_ci = [605,344,41]
vr_4min_ci = [1818,753,109]

mp_1min=[255,	1255,	13547]
mp_2min=[341,	1427,	13055]
mp_4min=[432,1717, 13508]
mp_1min_ci=[59,137,97]
mp_2min_ci=[87,175,1118]
mp_4min_ci=[153,229,78]

all_1min = [("Omni-Paxos", paxos_1min, paxos_1min_ci), ("Raft", raft_1min, raft_1min_ci), ("Raft PV+CQ", raft_pvqc_1min, raft_pvqc_1min_ci), ("VR", vr_1min, vr_1min_ci), ("Multi-Paxos", mp_1min, mp_1min_ci)]
all_2min = [("Omni-Paxos", paxos_2min, paxos_1min_ci), ("Raft", raft_2min, raft_2min_ci), ("Raft PV+CQ", raft_pvqc_2min, raft_pvqc_2min_ci), ("VR", vr_2min, vr_2min_ci), ("Multi-Paxos", mp_2min, mp_2min_ci)]
all_4min = [("Omni-Paxos", paxos_4min, paxos_1min_ci), ("Raft", raft_4min, raft_4min_ci), ("Raft PV+CQ", raft_pvqc_4min, raft_pvqc_4min_ci), ("VR", vr_4min, vr_4min_ci), ("Multi-Paxos", mp_4min, mp_4min_ci)]

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
			error_bar_color = "black"
			if util.colors[label] == "black":
				error_bar_color = "gray"
			#print("x: {}, y: {}".format(x, y))
			#plt.bar(x, y, align='edge', width=BAR_WIDTH, color=util.colors[label], alpha=0.95, hatch=util.textures[timeout])
			plt.bar(x, y, align='edge', width=BAR_WIDTH, color=util.colors[label], alpha=0.95, yerr = ci[algo_group_len - j-1]/1000, ecolor = error_bar_color, error_kw = {"elinewidth": 3.5})

#plt.yscale("log")


#patch50ms = mpatches.Patch(facecolor='white', hatch='\\\\', label=TIMEOUTS[0], alpha=0.9, edgecolor='black')
#patch500ms = mpatches.Patch(facecolor='white', hatch='..', label=TIMEOUTS[1], alpha=0.9, edgecolor='black')
#patch5000ms = mpatches.Patch(facecolor='white', hatch=util.textures[TIMEOUTS[2]], label=TIMEOUTS[2], alpha=0.9, edgecolor='black')

#ax.legend(handles=[patch50ms, patch500ms, patch5000ms], ncol=3)


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
fig.savefig("bar_constrained_new.pdf", dpi = 600, bbox_inches='tight')