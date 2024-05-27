set xlabel "Time"
set ylabel "Memory usage (MB)"
set title "Memory usage of process "
set xdata time
set timefmt "%s.%N" # Set time format to include nanoseconds
set format x "%H:%M:%S"
plot "memory_usage.log" using 1:($2/1024) with linespoints title "Memory Usage (MB)"
