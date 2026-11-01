GITIGNORE_PARTS := $(sort $(wildcard .gitignore.d/*.gitignore))

LICENSE_PACKAGES := packages/core packages/cli npm/darwin-arm64 npm/darwin-x64 npm/linux-arm64 npm/linux-x64
NOTICE_PACKAGES := npm/darwin-arm64 npm/darwin-x64 npm/linux-arm64 npm/linux-x64

.PHONY: check-gitignore check-licenses check-notice clean-dist licenses notice FORCE

FORCE:

# Copies the license files into the packages that npm publishes. The copies are
# ignored by git, and the CD workflow runs this before it publishes.
licenses:
	@set -e; for dir in $(LICENSE_PACKAGES); do cp LICENSE-MIT LICENSE-APACHE "$$dir/"; done
	@set -e; for dir in $(NOTICE_PACKAGES); do cp NOTICE "$$dir/"; done

# Packs every package that carries license files. Fails when a tarball lacks one of them, or
# declares another license than its files imply: a package with a NOTICE lists the licenses
# of its libraries.
check-licenses:
	@set -eu; tmp=$$(mktemp -d); \
	for dir in $(LICENSE_PACKAGES); do \
		tarball=$$(pnpm --dir "$$dir" pack --pack-destination "$$tmp" | tail -1); \
		files=$$(tar -tzf "$$tarball"); \
		required='LICENSE-MIT LICENSE-APACHE'; expected='(MIT OR Apache-2.0)'; \
		case " $(NOTICE_PACKAGES) " in *" $$dir "*) required="$$required NOTICE" ;; esac; \
		case "$$dir" in \
			npm/darwin-arm64|npm/darwin-x64) expected='(MIT OR Apache-2.0) AND BSD-2-Clause AND BSD-3-Clause AND MPL-2.0 AND Unicode-3.0 AND Zlib' ;; \
			npm/linux-arm64|npm/linux-x64) expected='(MIT OR Apache-2.0) AND BSD-2-Clause AND BSD-3-Clause AND CC0-1.0 AND ISC AND MPL-2.0 AND Unicode-3.0 AND Zlib' ;; \
		esac; \
		for file in $$required; do \
			echo "$$files" | grep -qx "package/$$file" || { echo "::error::$$dir: $$file is missing from the tarball"; exit 1; }; \
		done; \
		actual=$$(tar -xzOf "$$tarball" package/package.json | jq -r '.license'); \
		[ "$$actual" = "$$expected" ] || { echo "::error::$$dir: license is $$actual, expected $$expected"; exit 1; }; \
	done

# Rewrites the generated entries and license texts of NOTICE. Needs cargo and the crate sources.
notice:
	@node scripts/generate-notice.mts

# Fails when NOTICE, or the license of a platform package, is out of date.
check-notice:
	@node scripts/generate-notice.mts --check

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
