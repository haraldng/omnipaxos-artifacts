# Importing the necessary libraries and modules
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import numpy as np
import util

UNIT= 1000000
TIMEOUTS = ["50ms","500ms","5000ms"]
BAR_WIDTH= 0.75
BAR_DIST = 0.3
BASE = 1.7
GROUP_DIST = 10

fig, ax = plt.subplots()

paxos_1min = [5422813, 5730999, 5655222]
paxos_2min = [11013929, 10619087, 11726996]
paxos_4min = [21827108,	21946354,	22055321]
paxos_1min_ci = [327221,	151484,	88734]
paxos_2min_ci = [230134,	202887,	257918]
paxos_4min_ci = [507525,	231643,	360039]
paxos_4min_ci = [507525,	716199,	758824]


raft_1min = [5520543, 5495848, 5141543]
raft_2min = [10751857,	10958405,	9861789]
raft_4min = [20147452,	21787629,	21066741]
raft_1min_ci = [449534,144487,726553]
raft_2min_ci = [482312,481656,1270334]
raft_4min_ci = [1842112,651538,488261]

raft_pvqc_1min = [5273627,5699617,6103325]
raft_pvqc_2min = [10958283,11417252,11274339]
raft_pvqc_4min = [21098625,21451300,21015935]
raft_pvqc_1min_ci = [495906,163556,257741]
raft_pvqc_2min_ci = [126739,185956,119705]
raft_pvqc_4min_ci = [496714,618762,367369]

vr_1min = [5100339,5889021,6233647]
vr_2min = [11436142,10610071,11997877]
vr_4min = [21887426,22082772,21287473]
vr_1min_ci = [920843,202574,154388]
vr_2min_ci = [105267,590561,439732]
vr_4min_ci = [660156,395420,922557]

mp_1min=[3965846,4318244,4645184]
mp_2min=[7972283,8266536,8628067]
mp_4min=[15810896,15920373,16329512]
mp_1min_ci=[178816,65611,170935]
mp_2min_ci=[72989,443288,312644]
mp_4min_ci=[306065,313802,382454]

all_1min = [("Omni-Paxos", paxos_1min, paxos_1min_ci), ("Raft", raft_1min, raft_1min_ci), ("Raft PV+CQ", raft_pvqc_1min, raft_pvqc_1min_ci), ("VR", vr_1min, vr_1min_ci), ("Multi-Paxos", mp_1min, mp_1min_ci)]
all_2min = [("Omni-Paxos", paxos_2min, paxos_2min_ci), ("Raft", raft_2min, raft_2min_ci), ("Raft PV+CQ", raft_pvqc_2min, raft_pvqc_2min_ci), ("VR", vr_2min, vr_2min_ci), ("Multi-Paxos", mp_2min, mp_2min_ci)]
all_4min = [("Omni-Paxos", paxos_4min, paxos_4min_ci), ("Raft", raft_4min, raft_4min_ci), ("Raft PV+CQ", raft_pvqc_4min, raft_pvqc_4min_ci), ("VR", vr_4min, vr_4min_ci), ("Multi-Paxos", mp_4min, mp_4min_ci)]

all_groups = [("1 min", all_1min), ("2 min", all_2min), ("4 min", all_4min)]

x_ticks = []
x_labels = []


for k in range(len(all_groups)):
	group_base = (k+1) * GROUP_DIST
	(partition_duration, group) = all_groups[k]
	for i in range(len(group)):
		(algo, algo_group, ci) = group[i]
		base = group_base % len(group) + (i+1)*BASE
		algo_group_len = len(algo_group)
		for j in range(algo_group_len):
			timeout = TIMEOUTS[j]
			label = "{} {}".format(algo, timeout)
			bar_dist = (j)*BAR_DIST
			y = algo_group[j]
			#bar_dist = (j)*BAR_DIST
			#y = algo_group[j]
			x = base + bar_dist
			#print("x: {}, y: {}".format(x, y))
			#plt.bar(x, y, align='edge', width=BAR_WIDTH, color=util.colors[label], alpha=0.95, yerr = ci[algo_group_len - j-1]/1000, ecolor = error_bar_color, error_kw = {"elinewidth": 3.5})
			plt.errorbar(x, y/UNIT, yerr = ci[j]/UNIT, color=util.colors[label], elinewidth=3, zorder=1)
			plt.scatter(x, y/UNIT, color=util.colors[label], s=130, zorder=2, marker=util.markers[partition_duration], edgecolors="black")
			if j == 1:
				x_ticks.append(x)
				x_labels.append(algo)

#plt.yscale("log")
SIZE = 20
plt.rc('axes', labelsize=SIZE)    # fontsize of the x and y labels
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=SIZE)    # fontsize of the tick labels


ax.set_xticks(x_ticks)
ax.set_xticklabels(x_labels)
ax.tick_params(axis='both', which='major', labelsize=SIZE)
ax.set_ylabel("Number of Decided (million ops)", size=20)
#ax.ticklabel_format(scilimits=(0, 6))
#ax.set_xlabel("Partition Duration", size=20)
#ax.set_ylim(bottom=0)
#ax.legend(fontsize=18)
fig = ax.get_figure()
fig.set_size_inches(8, 4.5)
fig.savefig("chained-all.pdf", dpi = 600, bbox_inches='tight')