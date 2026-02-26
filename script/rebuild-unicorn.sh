#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# EMSDK_DIR="${EMSDK:-/Users/libr/Desktop/Life/emsdk}"
UNICORN_DIR="${UNICORN_DIR:-${ROOT_DIR}/../unicorn}"
UNICORN_BUILD_DIR="${UNICORN_BUILD_DIR:-${UNICORN_DIR}/build}"
JOBS="${JOBS:-8}"
PATCH_DIR="${PATCH_DIR:-${ROOT_DIR}/script/patches}"

if [[ ! -d "${UNICORN_DIR}" ]]; then
  echo "unicorn directory not found: ${UNICORN_DIR}"
  exit 1
fi

# if [[ -f "${EMSDK_DIR}/emsdk_env.sh" ]]; then
#   # shellcheck disable=SC1090
#   source "${EMSDK_DIR}/emsdk_env.sh" >/dev/null
# else
#   echo "emsdk_env.sh not found at ${EMSDK_DIR}/emsdk_env.sh"
#   exit 1
# fi

# if [[ -d "${PATCH_DIR}" ]]; then
#   for patch_file in "${PATCH_DIR}"/*.diff; do
#     if [[ ! -f "${patch_file}" ]]; then
#       continue
#     fi

#     echo "applying patch: ${patch_file}"
#     if ! git -C "${UNICORN_DIR}" apply "${patch_file}"; then
#       echo "skip failed patch: ${patch_file}"
#     fi
#   done
# fi

# rm -rf "${UNICORN_BUILD_DIR}"
mkdir -p "${UNICORN_BUILD_DIR}"

pushd "${UNICORN_BUILD_DIR}" >/dev/null
emcmake cmake "${UNICORN_DIR}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DUNICORN_BUILD_TESTS=OFF \
  -DUNICORN_INSTALL=OFF \
  -DUNICORN_LEGACY_STATIC_ARCHIVE=ON \
  -DUNICORN_INTERPRETER=ON \
  -DUNICORN_ARCH="arm;aarch64" \
  -DCMAKE_C_COMPILER=emcc \
  -DCMAKE_C_FLAGS="-DUSE_STATIC_CODE_GEN_BUFFER"
cmake --build . -- -j"${JOBS}"
popd >/dev/null

echo "unicorn rebuild done: ${UNICORN_BUILD_DIR}"
