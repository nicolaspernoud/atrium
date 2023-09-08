#!/bin/bash

# Check if gnuplot is installed
if ! command -v gnuplot &>/dev/null; then
    sudo apt-get install -y gnuplot
fi

# Check for the correct number of arguments
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <process_name>"
    exit 1
fi

process_name=$1

# Get the PID of the process by its name
pid=$(pgrep $process_name)

if [ -z "$pid" ]; then
    echo "No process found with the name '$process_name'."
    exit 1
fi

# Create the gnuplot script
gnuplot_script=$(
    cat <<EOL
set xlabel "Time"
set ylabel "Memory usage (MB)"
set title "Memory usage of process $process_name"
set xdata time
set timefmt "%s.%N" # Set time format to include nanoseconds
set format x "%H:%M:%S"
plot "memory_usage.log" using 1:(\$2/1024) with linespoints title "Memory Usage (MB)"
EOL
)

# Save the gnuplot script to a file
echo "$gnuplot_script" >plot_memory_usage.gp

run_loop() {
    while true; do
        mem=$(ps -p $pid -o rss=)
        timestamp=$(date +%s.%N)
        echo "$timestamp $mem" >>memory_usage.log
        sleep 0.1
    done
}

run_loop &

read -n 1 -s -r -p "Press any key to stop the loop."

# Kill the background process
kill $!

# Launch the gnuplot window to display the graph
sleep 2 && gnuplot -persist plot_memory_usage.gp

# Clean up
rm -f memory_usage.log plot_memory_usage.gp
