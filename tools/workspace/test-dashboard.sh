#!/usr/bin/env bash
# shellcheck disable=SC2034

set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$TEST_DIR/../.." && pwd)"

bash -n "$REPO_DIR/layerx"

export LAYERX_DASHBOARD_LIBRARY=1
# shellcheck source=/dev/null
source "$REPO_DIR/layerx"

fail() {
  printf 'dashboard test failed: %s\n' "$1" >&2
  exit 1
}

[[ ${#MODULE_IDS[@]} -eq 8 ]] || fail "expected all eight workspace modules"
[[ "$(selected_count)" == "8" ]] || fail "every module should be selected initially"

ACTION_INDEX=4
build_plan
[[ ${#STEP_MODULE[@]} -eq 52 ]] || fail "full plan should contain all 52 workspace steps"

for expected in core contracts agent human platform programs interop specgen; do
  found=0
  for actual in "${STEP_MODULE[@]}"; do
    [[ "$actual" == "$expected" ]] && found=1
  done
  [[ $found -eq 1 ]] || fail "full plan omitted $expected"
done

set_all_modules 0
SELECTED[4]=1
ACTION_INDEX=1
build_plan
[[ ${#STEP_MODULE[@]} -eq 6 ]] || fail "platform install should contain six dependency steps"
[[ "${STEP_COMMAND[0]}" == "cargo fetch --manifest-path platform/Cargo.toml --locked" ]] || fail "platform Rust dependency step drifted"
[[ "${STEP_COMMAND[5]}" == "swift package resolve" ]] || fail "platform Swift dependency step drifted"

set_all_modules 1
before="${SELECTED[0]}"
handle_click 5 8
[[ "${SELECTED[0]}" != "$before" ]] || fail "mouse click did not toggle the core module"

FOCUS=0
ENV_INDEX=0
EVENT=right
handle_dashboard_event
[[ $ENV_INDEX -eq 1 ]] || fail "keyboard environment selection failed"

FOCUS=11
ACTION_INDEX=0
EVENT=right
handle_dashboard_event
[[ $ACTION_INDEX -eq 1 ]] || fail "keyboard action selection failed"

set_all_modules 0
SELECTED[7]=1
ACTION_INDEX=4
DRY_RUN=1
build_plan
render_execution() { :; }
render_results() { :; }
read_event() { EVENT=""; }
run_plan
[[ -z "$RUN_DIRECTORY" ]] || fail "plan-only mode created a run directory"
for status in "${STEP_STATUS[@]}"; do
  [[ "$status" == "PLANNED" ]] || fail "plan-only mode did not preserve planned status"
done

printf 'dashboard checks passed\n'
