#!/usr/bin/env bash
# Releases the CLI: scripts/release.sh patch|minor|major|<version> [--yes]
#
# Sets the version in Cargo.toml, moves the changelog's Unreleased notes under the new
# version, runs the checks, then commits, tags and pushes. Pushing the tag starts
# .github/workflows/release.yml, which builds every platform, creates the GitHub release,
# and publishes the Homebrew formula and the npm package. This script is the local half.
set -Eeuo pipefail

cd "$(dirname "$0")/.."

usage() {
  echo "Usage: scripts/release.sh patch|minor|major|<version> [--yes]" >&2
  echo "  <version> is x.y.z, or a prerelease such as 0.2.0-rc.1 (GitHub release only)." >&2
  exit 1
}

bump="" yes=false
for arg in "$@"; do
  case "$arg" in
    --yes | -y) yes=true ;;
    -h | --help) usage ;;
    *)
      [[ -z "$bump" ]] || usage
      bump="$arg"
      ;;
  esac
done
[[ -n "$bump" ]] || usage

fail() {
  echo "error: $*" >&2
  exit 1
}

run() {
  echo "\$ $*"
  "$@"
}

# ---------- the tools and the repo must be ready ----------

dist="$(command -v dist || true)"
[[ -n "$dist" ]] || dist="${CARGO_HOME:-$HOME/.cargo}/bin/dist"
[[ -x "$dist" ]] || fail "cargo-dist isn't installed. Run: cargo install cargo-dist --locked"

branch="$(git rev-parse --abbrev-ref HEAD)"
[[ "$branch" == "main" ]] || fail "On branch \"$branch\". Switch to main first."
[[ -z "$(git status --porcelain)" ]] || fail "The working tree has changes. Commit or stash them first."
run git fetch -q origin main --tags
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] ||
  fail "Local main isn't the same as origin/main. Pull or push first."

# ---------- the next version ----------

current="$(awk -F'"' '/^\[package\]/ { p = 1 } p && /^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$current" ]] || fail "Couldn't read the version from Cargo.toml."

semver='^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.-]+)?$'
case "$bump" in
  patch | minor | major)
    [[ "$current" =~ $semver ]] || fail "Cargo.toml's version isn't x.y.z: $current"
    if [[ -n "${BASH_REMATCH[4]}" ]]; then
      fail "Cargo.toml's version is a prerelease ($current). Give the version to release instead."
    fi
    ma="${BASH_REMATCH[1]}" mi="${BASH_REMATCH[2]}" pa="${BASH_REMATCH[3]}"
    case "$bump" in
      patch) next="$ma.$mi.$((pa + 1))" ;;
      minor) next="$ma.$((mi + 1)).0" ;;
      major) next="$((ma + 1)).0.0" ;;
    esac
    ;;
  *)
    next="${bump#v}"
    [[ "$next" =~ $semver ]] || fail "Not a version: $bump"
    ;;
esac
[[ "$next" != "$current" ]] || fail "Cargo.toml is already at $next."
tag="v$next"
if git rev-parse -q --verify "refs/tags/$tag" > /dev/null; then fail "The tag $tag already exists."; fi
prerelease=false
[[ "$next" == *-* ]] && prerelease=true

# ---------- release notes ----------

# The lines under "## [Unreleased]", up to the next version heading.
unreleased="$(awk '/^## \[Unreleased\]/ { s = 1; next } s && /^## \[/ { exit } s' CHANGELOG.md)"
if [[ -z "${unreleased//[[:space:]]/}" ]]; then
  if $prerelease; then
    echo "note: CHANGELOG.md has nothing under [Unreleased]; the prerelease will have no notes."
  else
    fail "CHANGELOG.md has nothing under [Unreleased]. Add the release notes first."
  fi
fi

echo
echo "Release $current → $next ($tag)"
if $prerelease; then
  echo "A prerelease: GitHub release only. Homebrew and npm are skipped, and the changelog isn't changed."
else
  echo "Release notes:"
  echo "${unreleased}" | sed '/./,$!d' | sed 's/^/  /'
fi
echo

# ---------- checks, before anything changes ----------

run cargo fmt --check
run cargo clippy --all-targets --locked -- -D warnings
run cargo test --locked

if ! $yes; then
  read -r -p "Commit, tag and push $tag? [y/N] " answer
  [[ "$answer" == [yY] || "$answer" == [yY][eE][sS] ]] || fail "Stopped. Nothing was changed."
fi

# ---------- bump, commit, tag, push ----------

# Until the commit is made, a failure puts the files back as they were.
restore() { git checkout -q -- Cargo.toml Cargo.lock CHANGELOG.md; }
trap 'restore; echo "Put Cargo.toml, Cargo.lock and CHANGELOG.md back as they were." >&2' ERR

awk -v v="$next" '
  /^\[/ { p = ($0 == "[package]") }
  p && !done && /^version = / { print "version = \"" v "\""; done = 1; next }
  { print }
' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
# Puts the new version in Cargo.lock without updating any dependency.
run cargo update --workspace --quiet

if ! $prerelease; then
  date="$(date -u +%Y-%m-%d)"
  awk -v heading="## [$next] - $date" '
    /^## \[Unreleased\]/ { print; print ""; print heading; print ""; skip = 1; next }
    skip && /^[[:space:]]*$/ { next }
    { skip = 0; print }
  ' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
fi

# cargo-dist checks that the tag matches Cargo.toml and the release workflow is current.
# Its "can't find rustup" warnings don't matter here: CI does the cross-compiling.
run "$dist" plan --tag "$tag" > /dev/null 2> >(grep -v "find rustup" >&2)

run git add Cargo.toml Cargo.lock CHANGELOG.md
run git commit -q -m "chore: release $next"
trap - ERR
run git tag "$tag"
run git push --atomic origin main "$tag"

echo
echo "Pushed $tag. The release workflow now builds, makes the GitHub release, and publishes to Homebrew and npm:"
echo "  https://github.com/dotquad/cli/actions/workflows/release.yml"
