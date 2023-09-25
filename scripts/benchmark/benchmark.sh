#!/bin/bash

WD="$(
  cd "$(dirname "$0")"
  pwd -P
)"

mkdir $WD/reports
REPORT_FILE="$WD/reports/$(date +"%Y-%m-%d_%H:%M:%S")_test_atrium.txt"

PROXY=http://app1.atrium.127.0.0.1.nip.io:8180
BENCH_CMD="rewrk -c 400 -t 8 -d 20s -h ${PROXY} --pct >> $REPORT_FILE"

test_proxy() {
  if [ "$(curl -s ${PROXY})" != "Hello World!" ]; then
    echo "Error: curl command did not return 'Hello World!'"
    exit 1
  fi
}

pkill actix_backend
pkill atrium
docker stop nginx_bench
docker rm nginx_bench

#####################################################################
#                            INSTALL RWRK                           #
#####################################################################

sudo apt install -y libssl-dev
sudo apt install -y pkg-config
cargo install rewrk --git https://github.com/ChillFish8/rewrk.git

#####################################################################
#                              BACKEND                              #
#####################################################################

# Build for production
cd ${WD}/actix_backend
cargo build --release
# Start
${WD}/actix_backend/target/release/actix_backend &
BACKEND_PID=$!
sleep 2

#####################################################################
#                               NGINX                               #
#####################################################################

# Start proxy
docker run -d --name nginx_bench \
  -v ${WD}/nginx_default.conf:/etc/nginx/conf.d/default.conf \
  --net=host \
  nginx
sleep 2

# Test proxy
echo -e "#######################\n### NGINX IN DOCKER ###\n#######################\n" >>$REPORT_FILE
test_proxy
eval ${BENCH_CMD}

# Shutdown
docker stop nginx_bench
docker rm nginx_bench

#####################################################################
#                               ATRIUM                              #
#####################################################################

# Build for production
cd ${WD}/../../backend
cargo build --release
# Copy configuration
cp ${WD}/atrium.yaml ${WD}/../../backend/target/release/
# Start proxy
cd ${WD}/../../backend/target/release/
./atrium &
ATRIUM_PROXY_PID=$!
sleep 2
# Test proxy
echo -e "##############\n### ATRIUM ###\n##############\n" >>$REPORT_FILE
test_proxy
eval ${BENCH_CMD}
# Shutdown
kill $ATRIUM_PROXY_PID

#####################################################################
#                          ATRIUM IN DOCKER                         #
#####################################################################

# Build for production
cd ${WD}/../..
docker build --platform linux/amd64 -t atrium_bench .

# Start proxy
docker run -d --name atrium_bench \
  -v ${WD}/atrium.yaml:/app/atrium.yaml \
  --net=host \
  atrium_bench
sleep 2

# Test proxy
echo -e "########################\n### ATRIUM IN DOCKER ###\n########################\n" >>$REPORT_FILE
test_proxy
eval ${BENCH_CMD}

# Shutdown
docker stop atrium_bench
docker rm atrium_bench

#####################################################################
#                          BACKEND SHUTDOWN                         #
#####################################################################

# Shutdown backend
kill $BACKEND_PID

cat $REPORT_FILE
