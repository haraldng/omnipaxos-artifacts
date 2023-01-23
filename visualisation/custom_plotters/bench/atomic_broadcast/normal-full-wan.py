import numpy as np
import matplotlib.pyplot as plt
import util

FILENAME = "full-wan"
num_proposals = 20 * 1000000

def to_tp(t):
    return num_proposals/(t/1000)

def ci_to_err_tp(ci):
    (lo, hi) = ci
    (lo_tp, hi_tp) = (to_tp(hi), to_tp(lo))
    err = (hi_tp-lo_tp)/2
    return err

USE_N = 3
SIZE = 20
plt.rc('axes', labelsize=SIZE) 
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=SIZE)    # fontsize of the tick labels

wan_omni3_tp = [4739.406333333333, 46058.712, 123559.50366666667, 112544.17333333334]
wan_omni3_ci = [15.470612264356078, 207.05940470858332, 2753.5161259473825, 4002.3824670747417]
wan_omni5_tp = [4246.448888888889, 41015.74444444445, 124035.94777777778, 109295.1211111111]
wan_omni5_ci = [14.368222206855535, 2134.5822775615197, 3234.5822775615197, 3495.257587102038]

wan_raft3_tp = [4748.895925925925, 46071.3275, 123670.292, 113462.78888888888]
wan_raft3_ci = [30.309196757888458, 292.9214306488575, 1067.9833340525147, 4922.61204521326]
wan_raft5_tp = [4735.131111111111, 46016.21777777778, 129402.66666666667, 111019.0611111111]
wan_raft5_ci = [38.15901947527436, 2635.0126388350945, 3153.5161259473825, 2554.2584742654944]

wan_mp3_tp = [4790.140555555556, 47090.373888888884, 126120.97277777779, 111948.01888888888]
wan_mp3_ci = [4.536244895964046, 106.89095889486998, 3759.82915686068, 3099.7893847079104]
wan_mp5_tp = [4304.125555555555, 42329.75444444444, 122928.68444444446, 109558.8211111111]
wan_mp5_ci = [24.834247079988472, 1032.3030849719544, 3632.30308497195497, 631.1777452833339]

x_axis = [1, 2, 3 ,4]
num_cp = ["500", "5k", "50k", "500k"]

all_series = [
    ("Raft, n=3", wan_raft3_tp, wan_raft3_ci),
    ("Raft, n=5", wan_raft5_tp, wan_raft5_ci),
    ("Omni-Paxos, n=3", wan_omni3_tp, wan_omni3_ci),
    ("Omni-Paxos, n=5", wan_omni5_tp, wan_omni5_ci),
    ("Multi-Paxos, n=3", wan_mp3_tp, wan_mp3_ci),
    ("Multi-Paxos, n=5", wan_mp5_tp, wan_mp5_ci),
]

fig, ax = plt.subplots()
for (label, data, err) in all_series:
    color = util.colors[label]
    split = label.split(",")
    linestyle = util.linestyles[split[0]]
    marker = util.markers[split[1]]
    plt.errorbar(x_axis[:USE_N], data[:USE_N], label=label, color=color, marker=marker, linestyle=linestyle, yerr=err[:USE_N], capsize=8)

ax.yaxis.set_major_formatter(util.format_k)

#ax.legend(loc = "lower right", fontsize=10, ncol=1)
#fig.set_size_inches(15, 3.5)
#ax.set_ylabel('Throughput (ops/s)')
#ax.set_xlabel('Number of concurrent proposals')
plt.xticks(x_axis[:USE_N], num_cp[:USE_N])
plt.yticks(np.arange(0, 175000, 25000).tolist())
ax.set_ylim(bottom=0)
#plt.yticks([90000, 110000, 130000, 150000])
#ax.yaxis.set_major_formatter(util.format_k)

plt.savefig("{}.pdf".format(FILENAME), dpi = 600, bbox_inches='tight')