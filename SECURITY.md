# Security Policy

SOUL is an early-stage research prototype and does not currently publish production releases or a formal vulnerability disclosure program.

## Reporting a Vulnerability

Do not open a public issue for a suspected vulnerability. Contact the repository owner privately through GitHub and include:

- the affected component and version or commit;
- steps to reproduce;
- expected and observed behavior;
- potential impact;
- any suggested mitigation.

Do not include real personal data, credentials, private `.soul` packages, or unredacted database contents in a report.

## Security Boundary

The repository implements encrypted local storage, signed packages and receipts, scoped context compilation, deterministic policy evaluation, replay protections, and constrained integration interfaces. These controls do not imply universal control over third-party agents.

An action is governed only when its execution path is mediated by a SOUL-controlled component. The current Gateway is a local demonstration boundary; production connectors and external credential isolation remain future work.

## Supported Versions

No version is currently supported as a production release. Security fixes are applied to the default branch during active development.
