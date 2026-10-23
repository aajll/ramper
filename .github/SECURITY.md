# Security Policy

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository. Expect a response within 7 days. A confirmed issue is fixed in a patch release.

## Scope

`ramper` is a small `no_std` library with no dependencies, no `unsafe`, no allocation and no I/O. Its only dependency is the optional `libm` for a square root. Configuration is validated when set, an update cannot panic or loop on any time step, and the output never leaves the configured limits. Values large enough to overflow `f32` intermediates are described under numerical range in the design document; keeping inputs inside that range is the caller's concern.
