#!/bin/sh
set -eu

chown -R euoni:euoni /data
exec gosu euoni:euoni /usr/local/bin/euoni-converter
