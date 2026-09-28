#!/bin/sh
# Explicit failed-backend probe double in the disposable guest.
set -eu
test "$#" -eq 1
test "$1" = --version
exit 17
