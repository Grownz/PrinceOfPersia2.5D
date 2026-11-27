# Rules
- Builds for iteration (screenshots, checks, benchmarks of a change): `cargo build --profile release-fast -p pop-app` (exe in `target\release-fast\`, about 2 min incremental). The normal `--release` profile (fat LTO, about 8 min) only for releases (`tools/package.ps1`) and final speed measurements.
- Delegate every repo search and test run to the Haiku 5.5 subagents (search, grep, audit) to keep the main context small.
