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

data_files = [f for f in os.listdir(args.s) if f.endswith('.data')]
data_files.sort()
for filename in data_files :
	if "-240" in filename or "50ms" in filename:
		continue 
	print("------------------------------- {} ------------------------------- ".format(filename))
	for line in open(args.s + "/" + filename, 'r'):
		#print(line)
		splitted = line.split("|")
		print(splitted[0])
