#!/usr/bin/env bash
set -euo pipefail

readonly ARG_JOB="${1:-}"
readonly ARG_STEP="${2:-}"
readonly REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly APP_NAME="oh-my-lint"
readonly TARGET_DIR="${REPO_DIR}/target"
readonly DIST_DIR="${REPO_DIR}/dist"

SMOKE_DIR=""
FMT_CONFIG=""

function help() {
    echo "USAGE: ${0} job__check [step__fmt|step__clippy|step__test|step__build|step__smoke]"
    echo "       ${0} job__package <target>"
}

function cleanup() {
    if [[ -n "${SMOKE_DIR}" ]]; then
        rm -rf "${SMOKE_DIR}"
    fi
    if [[ -n "${FMT_CONFIG}" ]]; then
        rm -f "${FMT_CONFIG}"
    fi
}

trap cleanup EXIT

function step__fmt() {
    cd "${REPO_DIR}"
    # 仓库不带 rustfmt.toml；用空配置压掉父目录的 workspace 配置，保证本地与 CI 一致。
    FMT_CONFIG="$(mktemp)"
    local fmt_path="${FMT_CONFIG}"
    if command -v cygpath > /dev/null; then
        fmt_path="$(cygpath -w "${FMT_CONFIG}")"
    fi
    cargo fmt --all -- --check --config-path "${fmt_path}"
}

function step__clippy() {
    cd "${REPO_DIR}"
    cargo clippy --workspace --all-targets -- -D warnings
}

function step__test() {
    cd "${REPO_DIR}"
    cargo test --workspace
}

function step__build() {
    cd "${REPO_DIR}"
    cargo build --workspace --release
}

function step__smoke() {
    cd "${REPO_DIR}"
    local binary="${TARGET_DIR}/release/${APP_NAME}"
    if [[ ! -x "${binary}" ]]; then
        echo "error: missing ${binary}; run step__build first" >&2
        return 1
    fi
    SMOKE_DIR="$(mktemp -d)"
    local smoke_sql="${SMOKE_DIR}/fixture.sql"
    local smoke_json="${SMOKE_DIR}/check.json"
    local status=0
    printf 'select a, b, c, d from t where x = 1\n' > "${smoke_sql}"
    "${binary}" check "${smoke_sql}" --json > "${smoke_json}" || status=$?
    if [[ "${status}" -ne 1 ]]; then
        echo "error: expected exit code 1 for findings, got ${status}" >&2
        return 1
    fi
    local python_bin="python3"
    if ! command -v python3 > /dev/null; then
        python_bin="python"
    fi
    "${python_bin}" - "${smoke_json}" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as file:
    data = json.load(file)

findings = data.get("findings")
summary = data.get("summary")
if not isinstance(findings, list) or not isinstance(summary, dict):
    raise SystemExit("error: check --json must return {findings, summary}")
for finding in findings:
    missing = {"file", "line", "column", "rule", "severity", "message", "fixable"} - finding.keys()
    if missing:
        raise SystemExit(f"error: missing finding keys: {sorted(missing)}")
print("[step__smoke] PASS")
PY
}

function job__check() {
    case "${ARG_STEP}" in
    step__fmt) step__fmt ;;
    step__clippy) step__clippy ;;
    step__test) step__test ;;
    step__build) step__build ;;
    step__smoke) step__smoke ;;
    "")
        step__fmt
        step__clippy
        step__test
        step__build
        step__smoke
        ;;
    *)
        help
        return 1
        ;;
    esac
}

function job__package() {
    local target="${ARG_STEP}"
    if [[ -z "${target}" ]]; then
        help
        return 1
    fi
    cd "${REPO_DIR}"
    mkdir -p "${DIST_DIR}"
    if [[ "${target}" == *linux-musl ]]; then
        if ! command -v cross > /dev/null; then
            echo "error: cross is required for ${target}" >&2
            return 1
        fi
        cross build --release --target "${target}"
    else
        cargo build --release --target "${target}"
    fi

    local binary="${TARGET_DIR}/${target}/release/${APP_NAME}"
    local archive=""
    if [[ "${target}" == *windows* ]]; then
        binary="${binary}.exe"
        archive="${DIST_DIR}/${APP_NAME}-${target}.zip"
    else
        archive="${DIST_DIR}/${APP_NAME}-${target}.tar.gz"
    fi
    if [[ ! -f "${binary}" ]]; then
        echo "error: missing ${binary}" >&2
        return 1
    fi
    rm -f "${archive}"
    if [[ "${target}" == *windows* ]]; then
        local binary_win=""
        local archive_win=""
        binary_win="$(cygpath -w "${binary}")"
        archive_win="$(cygpath -w "${archive}")"
        powershell.exe -NoProfile -Command "Compress-Archive -Path '${binary_win}' -DestinationPath '${archive_win}' -Force"
    else
        COPYFILE_DISABLE=1 tar -czf "${archive}" -C "$(dirname "${binary}")" "${APP_NAME}"
    fi
    echo "[package] ${archive}"
}

case "${ARG_JOB}" in
job__check) job__check ;;
job__package) job__package ;;
*) help ;;
esac
