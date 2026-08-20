PLATFORM_CARGO ?= cargo
PLATFORM_MANIFEST := platform/Cargo.toml

.PHONY: platform-build platform-lint platform-test platform-sdk-check \
	platform-sdk-generate platform-release-check platform-release-plan

platform-build:
	$(PLATFORM_CARGO) build --manifest-path $(PLATFORM_MANIFEST) --locked --workspace
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-sdkgen -- --check
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-release -- --check platform/release/registries.kvx

platform-lint:
	$(PLATFORM_CARGO) clippy --manifest-path $(PLATFORM_MANIFEST) --locked --workspace --all-targets -- -D warnings
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-workspace-gate -- platform/workspace.kvx .
	sh platform/tools/dependency-policy.sh
	cargo deny --manifest-path $(PLATFORM_MANIFEST) check advisories bans sources

platform-test:
	$(PLATFORM_CARGO) test --manifest-path $(PLATFORM_MANIFEST) --locked --workspace

platform-sdk-check:
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-sdkgen -- --check

platform-sdk-generate:
	cargo run --manifest-path agent/tools/sdk-gen/Cargo.toml --locked -- --write
	cargo run --manifest-path human/tools/api-gen/Cargo.toml --locked -- human/schema/human-api human/apps/web/src/api/generated
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-sdkgen -- --write

platform-release-check:
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-release -- --check platform/release/registries.kvx

platform-release-plan:
	$(PLATFORM_CARGO) run --manifest-path $(PLATFORM_MANIFEST) --locked -p layerx-platform-release -- --plan platform/release/registries.kvx
