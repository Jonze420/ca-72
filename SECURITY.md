# Security

The CA-72 is an audio plug-in: it runs inside your host and reads and writes its presets
library. It connects to the internet only when you ask: **CHECK FOR UPDATES** in the presets'
drawer has your system's `curl` ask GitHub's API (`api.github.com`) for the CA-72's latest
release, sending nothing but that request; **DOWNLOAD** and **RELEASES PAGE** open the
CA-72's releases on GitHub in your browser. It never downloads or runs an installer itself.

If you find a security problem (for example a preset file or a host's saved state that
makes it crash, read or write outside its library, or an answer to the update check that
makes it open anything but the CA-72's releases), please report it privately through
GitHub's **Security › Report a vulnerability** on this repository
(<https://github.com/idlefoundry/ca-72/security/advisories/new>) rather than in a public
issue. Fixes go into the latest release.
