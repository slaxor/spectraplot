#!/usr/bin/env -S gnuplot -c

if (ARGC < 1) { print "usage: spectraplot.gp FILE"; exit status 1 }
file = ARG1

set datafile separator ","
stats file nooutput

set terminal sixelgd size 2400,1400
set title system(sprintf("sed -n '1s/^# *//p' '%s'", file)) noenhanced
set xlabel "nm"
set ylabel "PMT reading"
set grid

plot for [c=2:STATS_columns] file using 1:c with lines title sprintf("read %d", c - 1)
