#!/bin/sh
set -eu

sh ci/test.sh
sh ci/publish.sh
