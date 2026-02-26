#!/usr/bin/env bash
set -euo pipefail

BUILD_MODE="debug"
if [[ "${1:-}" == "--release" ]]; then
  BUILD_MODE="release"
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target/wasm32-unknown-emscripten/${BUILD_MODE}"
DIST_DIR="${ROOT_DIR}/dist"
# EMSDK_DIR="${EMSDK:-/Users/libr/Desktop/Life/emsdk}"
UNICORN_BUILD_DIR="${UNICORN_BUILD_DIR:-${ROOT_DIR}/../unicorn/build}"
NODE_DIST_JS="${DIST_DIR}/anisette_rs.node.js"
NODE_DIST_WASM="${DIST_DIR}/anisette_rs.node.wasm"



WEB_EXPORTED_FUNCTIONS='["_malloc","_free","_anisette_init_from_blobs","_anisette_is_machine_provisioned","_anisette_start_provisioning","_anisette_end_provisioning","_anisette_request_otp","_anisette_get_cpim_ptr","_anisette_get_cpim_len","_anisette_get_session","_anisette_get_otp_ptr","_anisette_get_otp_len","_anisette_get_mid_ptr","_anisette_get_mid_len","_anisette_last_error_ptr","_anisette_last_error_len","_anisette_fs_write_file","_anisette_idbfs_init","_anisette_idbfs_sync","_anisette_set_identifier","_anisette_set_provisioning_path"]'
NODE_EXPORTED_FUNCTIONS='["_malloc","_free","_anisette_init_from_blobs","_anisette_is_machine_provisioned","_anisette_start_provisioning","_anisette_end_provisioning","_anisette_request_otp","_anisette_get_cpim_ptr","_anisette_get_cpim_len","_anisette_get_session","_anisette_get_otp_ptr","_anisette_get_otp_len","_anisette_get_mid_ptr","_anisette_get_mid_len","_anisette_last_error_ptr","_anisette_last_error_len","_anisette_fs_write_file","_anisette_set_identifier","_anisette_set_provisioning_path"]'
WEB_EXPORTED_RUNTIME_METHODS='["FS","HEAPU8","UTF8ToString","stringToUTF8","lengthBytesUTF8"]'
NODE_EXPORTED_RUNTIME_METHODS='["HEAPU8","UTF8ToString","stringToUTF8","lengthBytesUTF8"]'

# if [[ -f "${EMSDK_DIR}/emsdk_env.sh" ]]; then
#   # shellcheck disable=SC1090
#   source "${EMSDK_DIR}/emsdk_env.sh" >/dev/null
# else
#   echo "emsdk_env.sh not found at ${EMSDK_DIR}/emsdk_env.sh"
#   exit 1
# fi

mkdir -p "${DIST_DIR}"

# if [[ "${SKIP_UNICORN_REBUILD:-0}" != "1" ]]; then
#   bash "${ROOT_DIR}/test/rebuild-unicorn.sh"
# fi

pushd "${ROOT_DIR}" >/dev/null
if [[ "${BUILD_MODE}" == "release" ]]; then
  cargo build --release --target wasm32-unknown-emscripten
else
  cargo build --target wasm32-unknown-emscripten
fi
popd >/dev/null

EMCC_INPUTS=(
  "${TARGET_DIR}/libanisette_rs.a"
  "${UNICORN_BUILD_DIR}/libunicorn.a"
  "${UNICORN_BUILD_DIR}/libunicorn-common.a"
  "${UNICORN_BUILD_DIR}/libaarch64-softmmu.a"
  "${UNICORN_BUILD_DIR}/libarm-softmmu.a"
)

for f in "${EMCC_INPUTS[@]}"; do
  if [[ ! -f "${f}" ]]; then
    echo "missing input: ${f}"
    exit 1
  fi
done

emcc \
  "${EMCC_INPUTS[@]}" \
  -lidbfs.js \
  -o "${DIST_DIR}/anisette_rs.js" \
  -sMODULARIZE=1 \
  -sEXPORT_ES6=1 \
  -sENVIRONMENT=web \
  -sWASM=1 \
  -sALLOW_MEMORY_GROWTH=1 \
  -sINITIAL_MEMORY=268435456 \
  -sWASM_BIGINT=1 \
  -sFORCE_FILESYSTEM=1 \
  -sASSERTIONS=1 \
  -sEXPORTED_FUNCTIONS="${WEB_EXPORTED_FUNCTIONS}" \
  -sEXPORTED_RUNTIME_METHODS="${WEB_EXPORTED_RUNTIME_METHODS}"

emcc \
  "${EMCC_INPUTS[@]}" \
  -o "${NODE_DIST_JS}" \
  -sMODULARIZE=1 \
  -sEXPORT_ES6=1 \
  -sENVIRONMENT=node \
  -sWASM=1 \
  -sALLOW_MEMORY_GROWTH=1 \
  -sINITIAL_MEMORY=268435456 \
  -sWASM_BIGINT=1 \
  -sFORCE_FILESYSTEM=0 \
  -sASSERTIONS=1 \
  -sEXPORTED_FUNCTIONS="${NODE_EXPORTED_FUNCTIONS}" \
  -sEXPORTED_RUNTIME_METHODS="${NODE_EXPORTED_RUNTIME_METHODS}"

echo "glue build done:"
echo "  ${DIST_DIR}/anisette_rs.js"
echo "  ${DIST_DIR}/anisette_rs.wasm"
echo "  ${NODE_DIST_JS}"
echo "  ${NODE_DIST_WASM}"

# Copy to frontend if directory exists (skip in CI if not present)
if [[ -d "${ROOT_DIR}/../../frontend/public/anisette" ]]; then
  cp "${DIST_DIR}/anisette_rs.js" "${ROOT_DIR}/../../frontend/public/anisette/anisette_rs.js"
  cp "${DIST_DIR}/anisette_rs.wasm" "${ROOT_DIR}/../../frontend/public/anisette/anisette_rs.wasm"
fi
