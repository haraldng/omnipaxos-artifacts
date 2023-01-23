import numpy as np
import matplotlib.pyplot as plt
import util

num_proposals = 10 * 1000000
def ms_to_tp(t):
    return num_proposals/(t/1000)

FILENAME = "chained"
TITLE = 'Chained scenario'
#raft = [50533.435782,52911.101637,60091.845166,65547.680951,54038.709471999995,74749.261094,90319.950333,60373.700495,60754.824064,69841.72998399999]
#raft_pv = [50377.20796,47050.350992,46920.739193999994,45494.902544,46634.340917999994,46939.709022999996,51820.560351,50758.268513999996,51059.424573,48731.035656]
#paxos = [50505.792305999996,50304.649215,50369.734166999995,49491.659029999995,49579.429575999995,49768.561979,49150.848915999995,49201.637051,49719.578301,49750.827285]
raft = [77477.47779199999,78847.124388,78507.288282,116608.468601,80013.386688,89233.329207,93541.08734499999,89051.650161,96543.26432399999,98074.504783]
raft_pv = [72442.206101,71423.60509699999,71875.104123,71349.615117,71852.505615,72757.10694,71685.49292799999,72958.02013599999,70568.877389,72227.767177]
paxos = [76470.658627,73879.055245,75433.500837,74833.16629899999,75510.869626,73816.560403,73272.779117,74754.283724,76651.79250899999,77017.862152]
vr = [72797.402515,74151.300733,74976.217856,69799.835826,68640.36134799999,71895.649359,71304.148041,73961.926476,73934.824326,72464.712665]
multi_paxos = [92515.432313,90032.31438099999,89516.98370099999,90795.718197,91262.79811599999,92736.50624799999,90672.981787,93042.317341,96248.291551,96573.942192]

raft_s = list(map(ms_to_tp, raft))
raft_pv_s = list(map(ms_to_tp, raft_pv))
paxos_s = list(map(ms_to_tp, paxos))
vr_s = list(map(ms_to_tp, vr))
multi_paxos_s = list(map(ms_to_tp, multi_paxos))

my_dict = {'MP': multi_paxos_s, 'Omni-Paxos': paxos_s, 'Raft': raft_s, 'Raft PV+CQ': raft_pv_s, 'VR': vr_s, }

y_axis = np.arange(0, 160000, 30000)

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
ax.set_yticks(y_axis, size=MEDIUM_SIZE)
ax.yaxis.set_major_formatter(util.format_k)

colors = ["tab:red", "tab:blue", "tab:purple", "tab:orange", "tab:green"]
# fill with colors
for patch, color in zip(bplot['boxes'], colors):
    patch.set_facecolor(color)

plt.savefig("{}.pdf".format(FILENAME), dpi = 600, bbox_inches='tight')