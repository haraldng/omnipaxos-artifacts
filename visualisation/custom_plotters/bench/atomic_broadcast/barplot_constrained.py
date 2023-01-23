import matplotlib
matplotlib.use('Agg')

import util
import matplotlib.pyplot as plt
import sys
import argparse
import os
import numpy as np
import datetime
import scipy.stats as st
import pandas as pd
from pathlib import Path
from matplotlib.ticker import (MultipleLocator,
                               FormatStrFormatter,
                               AutoMinorLocator)

def get_label_and_color(filename, dirname):
	csv = filename.split(",")
	dir_split = dirname.split("-")
	algorithm = csv[0]
	if algorithm == "paxos":
		algorithm = "Omni-Paxos"
	elif algorithm == "vr":
		algorithm = "VR"
	elif algorithm == "multi-paxos":
		algorithm = "Multi-Paxos"	
	elif algorithm == "raft":
		algorithm = "Raft"
	else:
		algorithm = "Raft PV+CQ"
	reconfig = csv[len(csv)-2].split(".")[0]
	if reconfig == "none":
		label = algorithm
	else:
		label = "{} {}".format(algorithm, reconfig.replace("-", " "))

	minutes = dir_split[1]
	#label = label + " {} min".format(minutes)
	color = util.colors[label]
	return (label, color)

def get_exp_duration(dirname):
	dirname.split("-")[1]


parser = argparse.ArgumentParser()

#parser.add_argument('-s', required=True, help='Directory of raw results')
parser.add_argument('-t', nargs='?', help='Output directory')
parser.add_argument('-w', required=True, type=int, help='Window duration (s)')
parser.add_argument('--no-ci', dest='ci', action='store_false')
parser.set_defaults(feature=True)

args = parser.parse_args()
print("Plotting with args:",args)

fig, ax = plt.subplots()

SIZE = 20
plt.rc('axes', labelsize=SIZE)    # fontsize of the x and y labels
plt.rc('xtick', labelsize=SIZE)    # fontsize of the tick labels
plt.rc('ytick', labelsize=SIZE)    # fontsize of the tick labels

max_ts = 0

directories = ["constrained-1-min", "constrained-2-min", "constrained-4-min"]

paxos = []
raft_pv_cq = []
vr = []
multi_paxos = []
raft = []

paxos_ci_lo = []
raft_pv_cq_ci_lo = []
vr_ci_lo = []
multi_paxos_ci_lo = []
raft_ci_lo = []

paxos_ci_hi = []
raft_pv_cq_ci_hi = []
vr_ci_hi = []
multi_paxos_ci_hi = []
raft_ci_hi = []

for d in directories:	# per duration
	full_dir = "/Users/haraldng/code/2022-05-16-plots/constrained_election/{}".format(d)
	data_files = [f for f in os.listdir(full_dir) if f.endswith('.data')]

	bar_group = []	# (filename, mean, (ci95lo, ci95hi))


	for filename in data_files :	# algo
		f = open(full_dir + "/" + filename, 'r')
		print("Reading", filename, "...")
		line_nr = 0
		all_recovery_duration = []
		for line in f:	# run
			line_nr = line_nr + 1
			print("Reading line ", line_nr)
			first_zero_window_idx = 0
			num_decided_per_window = line.split(",")
			for window_idx, num_decided in enumerate(num_decided_per_window):
				if num_decided.isdigit():
					num_decided = int(num_decided)
					if num_decided == 0 and first_zero_window_idx == 0:
						first_zero_window_idx = window_idx
					elif num_decided > 0 and first_zero_window_idx > 0:
						recovery_duration = (window_idx - first_zero_window_idx) * args.w 
						all_recovery_duration.append(recovery_duration)
						break
		print(d, filename)
		print(all_recovery_duration)
		avg_recovery = np.mean(np.array(all_recovery_duration))
		(ci_lo, ci_hi) = st.t.interval(alpha=0.95, df=len(all_recovery_duration)-1, loc=avg_recovery, scale=st.sem(np.array(all_recovery_duration)))
		if np.isnan(ci_lo):
			ci_lo = 0
		else:
			ci_lo = avg_recovery - ci_lo
		if np.isnan(ci_hi):
			ci_hi = 0
		else:
			ci_hi = ci_hi - avg_recovery
		(algo, color) = get_label_and_color(filename, d)

		if algo == "Omni-Paxos":
			paxos.append(avg_recovery)
			paxos_ci_lo.append(ci_lo)
			paxos_ci_hi.append(ci_hi)
		elif algo == "VR":
			vr.append(avg_recovery)
			vr_ci_lo.append(ci_lo)
			vr_ci_hi.append(ci_hi)
		elif algo == "Multi-Paxos":
			multi_paxos.append(avg_recovery)	
			multi_paxos_ci_lo.append(ci_lo)
			multi_paxos_ci_hi.append(ci_hi)
		elif algo == "Raft":
			raft.append(avg_recovery)
			raft_ci_lo.append(ci_lo)
			raft_ci_hi.append(ci_hi)
		elif algo == "Raft PV+CQ":
			raft_pv_cq.append(avg_recovery)
			raft_pv_cq_ci_lo.append(ci_lo)
			raft_pv_cq_ci_hi.append(ci_hi)

all_min = [paxos[0], raft_pv_cq[0], vr[0], multi_paxos[0], raft[0],
	   	   paxos[1], raft_pv_cq[1], vr[1], multi_paxos[1], raft[1],
	   	   paxos[2], raft_pv_cq[2], vr[2], multi_paxos[2], raft[2]]

all_ci_lo = [paxos_ci_lo[0], raft_pv_cq_ci_lo[0], vr_ci_lo[0], multi_paxos_ci_lo[0], raft_ci_lo[0],
	     	 paxos_ci_lo[1], raft_pv_cq_ci_lo[1], vr_ci_lo[1], multi_paxos_ci_lo[1], raft_ci_lo[1],
	     	 paxos_ci_lo[2], raft_pv_cq_ci_lo[2], vr_ci_lo[2], multi_paxos_ci_lo[2], raft_ci_lo[2]]

all_ci_hi = [paxos_ci_hi[0], raft_pv_cq_ci_hi[0], vr_ci_hi[0], multi_paxos_ci_hi[0], raft_ci_hi[0],
	     	 paxos_ci_hi[1], raft_pv_cq_ci_hi[1], vr_ci_hi[1], multi_paxos_ci_hi[1], raft_ci_hi[1],
	     	 paxos_ci_hi[2], raft_pv_cq_ci_hi[2], vr_ci_hi[2], multi_paxos_ci_hi[2], raft_ci_hi[2]]


dfdict = {'Partition Duration': ['1 min', '1 min', '1 min', '1 min', '1 min',
 				 '2 min', '2 min', '2 min', '2 min', '2 min',
 				 '4 min', '4 min', '4 min', '4 min', '4 min'],
      '': ['Omni-Paxos', 'Raft PV+CQ', 'VR', 'Multi-Paxos', 'Raft',
      	   'Omni-Paxos', 'Raft PV+CQ', 'VR', 'Multi-Paxos', 'Raft',
      	   'Omni-Paxos', 'Raft PV+CQ', 'VR', 'Multi-Paxos', 'Raft'],
      'Duration': all_min,
      'CiLo': all_ci_lo,
      'CiHi': all_ci_hi}

df = pd.DataFrame(dfdict)
errLo = df.pivot(index='Partition Duration', columns='', values='CiLo')
errHi = df.pivot(index='Partition Duration', columns='', values='CiHi')

err = []
for col in errLo:  # Iterate over bar groups (represented as columns)
    err.append([errLo[col].values, errHi[col].values])
print(err)

# sorted order: Multi-Paxos, Omni-Paxos, Raft, Raft PV+CQ, VR
ax = df.pivot(index='Partition Duration', columns='', values='Duration').plot(kind='bar', alpha=.99, yerr=err, error_kw=dict(lw=2, capsize=3, capthick=2)
,color = ["tab:red", "tab:blue", "tab:purple", "tab:orange", "tab:green"])

bars = ax.patches
patterns =['\\', '-', '/','+','//']
hatches = [p for p in patterns for i in range(len(df))]
for i, bar in enumerate(bars):
	if i < 3:
		hatch = patterns[0]
	elif i < 6:
		hatch = patterns[1]
	elif i < 9:
		hatch = patterns[2]
	elif i < 12:
		hatch = patterns[3]
	elif i < 15:
		hatch = patterns[4]
	bar.set_hatch(hatch)

#ax.set_ylabel("Down-time (s)")
y_axis = np.arange(0, 260, 50)
ax.set_yticks(y_axis, labels=[])
ax.set_xticklabels(ax.get_xticklabels(), rotation=0)
ax.get_legend().remove()
fig = ax.get_figure()
fig.set_size_inches(8.5, 4.5)
fig.savefig("bar_constrained.pdf", dpi = 600, bbox_inches='tight')

