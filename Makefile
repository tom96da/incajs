GITIGNORE_PARTS := $(sort $(wildcard .gitignore.d/*.gitignore))

LICENSE_PACKAGES := packages/core packages/cli npm/darwin-arm64 npm/darwin-x64 npm/linux-arm64 npm/linux-x64
NOTICE_PACKAGES := npm/darwin-arm64 npm/darwin-x64 npm/linux-arm64 npm/linux-x64

.PHONY: check-gitignore check-licenses clean-dist licenses FORCE

FORCE:

# Copies the license files into the packages that npm publishes. The copies are
# ignored by git, and the CD workflow runs this before it publishes.
licenses:
	@set -e; for dir in $(LICENSE_PACKAGES); do cp LICENSE-MIT LICENSE-APACHE "$$dir/"; done
	@set -e; for dir in $(NOTICE_PACKAGES); do cp NOTICE "$$dir/"; done

# Packs every package that carries license files. Fails when a tarball lacks one of them, or
# declares another license than its files imply: a package with a NOTICE contains MPL-2.0 libraries.
check-licenses:
	@set -eu; tmp=$$(mktemp -d); \
	for dir in $(LICENSE_PACKAGES); do \
		tarball=$$(pnpm --dir "$$dir" pack --pack-destination "$$tmp" | tail -1); \
		files=$$(tar -tzf "$$tarball"); \
		required='LICENSE-MIT LICENSE-APACHE'; expected='(MIT OR Apache-2.0)'; \
		case " $(NOTICE_PACKAGES) " in *" $$dir "*) required="$$required NOTICE"; expected='(MIT OR Apache-2.0) AND MPL-2.0' ;; esac; \
		for file in $$required; do \
			echo "$$files" | grep -qx "package/$$file" || { echo "::error::$$dir: $$file is missing from the tarball"; exit 1; }; \
		done; \
		actual=$$(tar -xzOf "$$tarball" package/package.json | jq -r '.license'); \
		[ "$$actual" = "$$expected" ] || { echo "::error::$$dir: license is $$actual, expected $$expected"; exit 1; }; \
	done

clean-dist:
	@find packages examples docs -type d -name dist -not -path '*/node_modules/*' -printf 'remove %p\n' -exec rm -rf {} +

.gitignore: FORCE $(GITIGNORE_PARTS)
	@printf '%s\n\n' '# This file is generated from .gitignore.d/*.gitignore.' > $@
	@first=1; for part in $(GITIGNORE_PARTS); do \
		if [ "$$first" -eq 0 ]; then printf '\n'; fi; \
		cat "$$part"; \
		first=0; \
	done >> $@

check-gitignore: .gitignore
	@git ls-files --error-unmatch .gitignore > /dev/null
	@if ! git diff --quiet -- .gitignore; then \
		printf '%s\033[1;31m%s\033[0m\n' 'The generated .gitignore has changed. Run: ' 'git add .gitignore'; \
		exit 1; \
	fi
	@printf '%s\n' 'The generated .gitignore is up to date.'
