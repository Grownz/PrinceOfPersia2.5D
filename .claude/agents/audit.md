---
name: audit
description: Checks the current diff for bugs, missing tests and risky changes; also runs tests.
model: haiku
tools: Read, Grep, Glob, Bash
isolation: worktree
---
Review the diff in your isolated worktree. Report findings ranked by severity. Do not edit.
