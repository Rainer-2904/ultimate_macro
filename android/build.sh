#!/usr/bin/env bash
set -euo pipefail
# Respect caller paths; defaults match the command-line toolchain installation.
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
export JAVA_HOME="${JAVA_HOME:-$HOME/Android/build-tools-host/jdk-17}"
export PATH="$JAVA_HOME/bin:$HOME/.cargo/bin:$PATH"
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
if [ "$#" -eq 0 ]; then set -- assembleDebug; fi
exec ./gradlew "$@"
