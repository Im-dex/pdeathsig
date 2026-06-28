# pdeathsig

`pdeathsig` is a tiny Linux-only wrapper that binds an executed command to the lifetime of its parent process.

It calls `prctl(PR_SET_PDEATHSIG, SIGTERM)` and then replaces itself with the target command using `exec`.

## Why

Python can set `PR_SET_PDEATHSIG` with `subprocess.Popen(preexec_fn=...)`, but `preexec_fn` is risky in multithreaded Python programs because it runs after process creation and before `exec`.

This wrapper moves the `prctl` call into a fresh standalone process:

```text
parent process
  -> pdeathsig <command> [args...]
       -> prctl(PR_SET_PDEATHSIG, SIGTERM)
       -> exec <command> [args...]
```

After `exec`, the PID stays the same, so the target command keeps the parent-death signal setting.

## Usage

```bash
pdeathsig restic snapshots
```

From Python:

```python
import os
import subprocess

proc = subprocess.Popen(
    ["pdeathsig", *cmd],
    start_new_session=True,
    env={
        **os.environ,
        "PDEATHSIG_EXPECTED_PARENT_PID": str(os.getpid()),
    },
)
```

`start_new_session=True` is not required for `PR_SET_PDEATHSIG`; it is useful when the parent also wants to terminate the whole process group during normal cleanup.

## Race protection

If `PDEATHSIG_EXPECTED_PARENT_PID` is set, `pdeathsig` verifies that its current parent PID matches the expected parent before executing the target command. This closes the race where the original parent dies before `pdeathsig` has installed `PR_SET_PDEATHSIG`.

If the variable is not set, `pdeathsig` uses the parent PID observed at startup.

## Exit codes

- `2`: invalid usage or invalid environment value
- `126`: command found but could not be executed
- `127`: `prctl` failed or command was not found

## Development

This project pins Rust with `rust-toolchain.toml` and pins direct dependencies in `Cargo.toml`.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Platform

Linux only.

## License

MIT.
