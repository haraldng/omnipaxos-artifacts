import numpy as np
import matplotlib.pyplot as plt
import util

FILENAME = "normal"
num_proposals = 20 * 1000000

def to_tp(t):
    return num_proposals/(t/1000)

def ci_to_err_tp(ci):
    (lo, hi) = ci
    (lo_tp, hi_tp) = (to_tp(hi), to_tp(lo))
    err = (hi_tp-lo_tp)/2
    return err


SIZE = 20
plt.rc('axes', labelsize=SIZE) 
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=SIZE)    # fontsize of the tick labels


lan_omni3_tp = [102062.08066666666, 143143.862, 132035.64133333333]
lan_omni3_ci = [4447.615266196393, 5224.539050566949, 5168.231962117177]

lan_raft3_tp = [104671.91166666667, 139802.34066666669, 128513.32933333333]
lan_raft3_ci = [3842.104912301147, 4564.7667389716225, 6313.095797843445]


lan_mp3_tp = [122957.08400000002, 137685.64333333334,132472.99454545457]
lan_mp3_ci = [4705.2702779142055, 11695.787102763083, 5995.387964216301]

lan_omni5_tp = [98012.73511111112, 133779.36911111113, 120607.60088888889]
lan_omni5_ci = [3195.5297764185816, 3943.040015817016, 5460.685064127771]

lan_raft5_tp = [102326.02212121213, 130372.47966666667, 123498.42099999999]
lan_raft5_ci = [3352.552344690659, 6343.666940461917, 6089.316293140626]

lan_mp5_tp = [114168.27200000001, 131468.69615384616, 120769.335]
lan_mp5_ci = [5991.661904329936, 7312.258087027149, 4508.017746045436]

x_axis = [1, 2, 3]
num_cp = ["500", "5k", "50k"]

all_series = [
    ("Raft, n=3", lan_raft3_tp, lan_raft3_ci),
    ("Raft, n=5", lan_raft5_tp, lan_raft5_ci),
    ("Omni-Paxos, n=3", lan_omni3_tp, lan_omni3_ci),
    ("Omni-Paxos, n=5",lan_omni5_tp, lan_omni5_ci),
    ("Multi-Paxos, n=3", lan_mp3_tp, lan_mp3_ci),
    ("Multi-Paxos, n=5",lan_mp5_tp, lan_mp5_ci)
]

fig, ax = plt.subplots()
for (label, data, err) in all_series:
    color = util.colors[label]
    split = label.split(",")
    linestyle = util.linestyles[split[0]]
    marker = util.markers[split[1]]
    plt.errorbar(x_axis, data, label=label, color=color, marker=marker, linestyle=linestyle, yerr=err, capsize=8)

ax.yaxis.set_major_formatter(util.format_k)

ax.legend(loc = "lower right", fontsize=19, ncol=1)
#ax.legend()
#fig.set_size_inches(15, 3.5)
ax.set_ylabel('Throughput (ops/s)')
ax.set_xlabel('Number of concurrent proposals')
ax.set_ylim(bottom=0)

plt.xticks(x_axis, num_cp)
plt.yticks(np.arange(0, 175000, 25000).tolist())
#plt.yticks([90000, 110000, 130000, 150000])
#ax.yaxis.set_major_formatter(util.format_k)

plt.savefig("{}.pdf".format(FILENAME), dpi = 600, bbox_inches='tight')