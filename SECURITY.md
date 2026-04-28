# Security

READZIP runs locally and does not send source code over the network.

## Reporting

Please report security issues privately through GitHub security advisories once the repository is public.

## Local Data

READZIP records intercept stats locally at `~/.cache/readzip/stats.tsv`: timestamp, hashed file path, original token estimate, and skeleton token estimate. The hashed path is a 64-bit Rust `DefaultHasher` digest, not the original path. Nothing is ever transmitted off the machine. Wipe with `rm -rf ~/.cache/readzip/stats.tsv`.

