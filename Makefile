GITIGNORE_PARTS := $(sort $(wildcard .gitignore.d/*.gitignore))

LICENSE_PACKAGES := packages/core packages/cli npm/darwin-arm64 npm/darwin-x64 npm/linux-arm64 npm/linux-x64
LICENSE_TARGETS := $(addsuffix /LICENSE,$(LICENSE_PACKAGES))

.PHONY: check-gitignore check-license clean-dist FORCE

FORCE:

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

# npm only includes a LICENSE* from the package's own directory; the template
# stays out of the repo root, where GitHub would read it as ours.
$(LICENSE_TARGETS): %/LICENSE: .github/license.tpl
	@cp $< $@

check-license: $(LICENSE_TARGETS)
	@for f in $(LICENSE_TARGETS); do \
		git ls-files --error-unmatch "$$f" > /dev/null || { \
			printf '%s\033[1;31m%s\033[0m\n' 'Missing generated file: ' "$$f (run: make $(LICENSE_TARGETS))"; \
			exit 1; \
		}; \
		if ! git diff --quiet -- "$$f"; then \
			printf '%s\033[1;31m%s\033[0m\n' 'The generated LICENSE files have changed. Run: ' 'git add $(LICENSE_TARGETS)'; \
			exit 1; \
		fi; \
	done
	@printf '%s\n' 'The generated LICENSE files are up to date.'
