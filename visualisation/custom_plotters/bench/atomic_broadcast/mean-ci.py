#!/usr/bin/env python
import sys
import argparse
import os

from scipy.stats import t
from numpy import average, std
from math import sqrt

def flush(data):
	mean = average(data)
	stddev = std(data, ddof=1)
	t_bounds = t.interval(0.95, len(data) - 1)
	ci = [mean + critval * stddev / sqrt(len(data)) for critval in t_bounds]
	a = ((ci[1]/300)-(ci[0]/300))/2
	return (mean/300, a)

parser = argparse.ArgumentParser()

parser.add_argument('-s', required=True, help='Directory of raw results')

args = parser.parse_args()
print("Plotting with args:",args)

data_files = [f for f in os.listdir(args.s) if f.endswith('summary.data')]
for filename in data_files :
	print("------------------------------- {} ------------------------------- ".format(filename))
	current_exp = ""
	data = []
	all_mean_tp = []
	all_ci_tp = []
	for line in open(args.s + "/" + filename, 'r'):
		#print(line)
		splitted = line.split("|")
		#print(splitted)
		if len(splitted) > 1:
			num_decided = float(splitted[0])
			warmup_decided = splitted[1]
			data.append(num_decided)
		else:
			#print(current_exp.strip())
			current_exp = line
			if len(data) > 0:
				(mean_tp, ci_tp) = flush(data)
				all_mean_tp.append(mean_tp)
				all_ci_tp.append(ci_tp)
				data = []
	#print(current_exp.strip())
	(mean_tp, ci_tp) = flush(data)
	all_mean_tp.append(mean_tp)
	all_ci_tp.append(ci_tp)
	print("ALL_MEAN_TP:\n{}".format(all_mean_tp))
	print("ALL_CI_TP:\n{}".format(all_ci_tp))
