#!/bin/sh
cd "$(dirname "$0")"
exec ./motor --sandbox "$@"
