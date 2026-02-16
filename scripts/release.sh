#!/bin/bash
# Release script for SRT Rust
# Usage: ./scripts/release.sh [major|minor|patch] [--dry-run]

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
BUMP_TYPE="${1:-patch}"
DRY_RUN=false
if [[ "$2" == "--dry-run" ]]; then
  DRY_RUN=true
fi

# Validate bump type
if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
  echo -e "${RED}Error: Invalid bump type '$BUMP_TYPE'. Use: major, minor, or patch${NC}"
  exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo -e "${YELLOW}Current version: $CURRENT_VERSION${NC}"

# Calculate new version
IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

case $BUMP_TYPE in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  patch)
    PATCH=$((PATCH + 1))
    ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
echo -e "${GREEN}New version: $NEW_VERSION${NC}"

if [ "$DRY_RUN" = true ]; then
  echo -e "${YELLOW}Dry run - no changes will be made${NC}"
  exit 0
fi

# Confirm
read -p "Proceed with release v$NEW_VERSION? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Aborted."
  exit 1
fi

# Update version in all Cargo.toml files
echo "Updating Cargo.toml files..."
find . -name "Cargo.toml" -not -path "./target/*" -exec sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" {} \;
find . -name "*.bak" -delete

# Update Cargo.lock
echo "Updating Cargo.lock..."
cargo update --workspace

# Run tests
echo "Running tests..."
cargo test --workspace

# Create git commit
echo "Creating git commit..."
git add .
git commit -m "chore: bump version to v$NEW_VERSION"

# Create git tag
echo "Creating git tag..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo ""
echo -e "${GREEN}✓ Release v$NEW_VERSION prepared!${NC}"
echo ""
echo "Next steps:"
echo "  1. Review the changes: git show"
echo "  2. Push the commit: git push origin master"
echo "  3. Push the tag: git push origin v$NEW_VERSION"
echo ""
echo "The GitHub Actions release workflow will automatically:"
echo "  - Build binaries for all platforms"
echo "  - Create a GitHub release"
echo "  - Upload release assets"
echo "  - Publish to crates.io (if configured)"
