import numpy as np
import matplotlib.pyplot as plt
import util

EXP_DURATION = 60

def decided_to_tp(n):
    return n/EXP_DURATION

FILENAME = "chained"
TITLE = 'Chained scenario'
raft = [4210006,2656229,3898714,756356,3670063, 3239200, 3802897, 3691521, 1428828, 3647779]
paxos = [3683645, 3567004, 3724231, 3478448, 3561144, 3714732, 3236195]
vr = [3426767, 3481193, 3670024, 3428600, 3467165, 3237648, 3670019, 3538641, 3680509]
raft_pv = [3950845, 3764188, 3623671, 3613019]



raft_s = list(map(decided_to_tp, raft))
raft_pv_s = list(map(decided_to_tp, raft_pv))
paxos_s = list(map(decided_to_tp, paxos))
vr_s = list(map(decided_to_tp, vr))
#multi_paxos_s = list(map(decided_to_tp, multi_paxos))


#my_dict = {'MP': multi_paxos_s, 'Omni-Paxos': paxos_s, 'Raft': raft_s, 'Raft PV+CQ': raft_pv_s, 'VR': vr_s, }
my_dict = {'Omni-Paxos': paxos_s, 'Raft': raft_s, 'Raft PV+CQ': raft_pv_s, 'VR': vr_s, }

#y_axis = np.arange(0, 160000, 30000)

MEDIUM_SIZE = 18
SIZE = 16
plt.rc('axes', labelsize=SIZE) 
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=MEDIUM_SIZE)    # fontsize of the tick labels


fig, ax = plt.subplots()
#fig.set_size_inches(10.3, 6)
fig.set_size_inches(8, 5.67)
#ax.set_title(TITLE, fontsize=MEDIUM_SIZE)
#bplot = ax.boxplot(my_dict.values())
bplot = ax.boxplot(my_dict.values(), patch_artist=True)
#ax.boxplot(my_dict.values(), positions=[1, 1.6, 2.2])
ax.set_xticklabels(my_dict.keys())
ax.set_ylabel('Throughput (ops/s)', size=22)
#ax.set_yticks(y_axis, size=MEDIUM_SIZE)
ax.yaxis.set_major_formatter(util.format_k)

colors = ["tab:red", "tab:blue", "tab:purple", "tab:orange", "tab:green"]
# fill with colors
for patch, color in zip(bplot['boxes'], colors):
    patch.set_facecolor(color)

plt.savefig("{}.pdf".format(FILENAME), dpi = 600, bbox_inches='tight')