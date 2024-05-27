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
