#!/bin/sh
set -eu

sh cicd/test.sh
sh cicd/publish.sh "$@"
