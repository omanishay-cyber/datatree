# Index your project

To make mneme useful, point it at your workspace and let it build the code graph.

The **Mneme: Build current workspace** command spawns `mneme build .` for you.
Output streams to the `Mneme` output channel.

Typical first-build numbers:

- **~10 MB** source tree: a few seconds.
- **~100 MB** source tree: under a minute.
- **Monorepo (~1 GB)**: a couple of minutes.

Subsequent builds are incremental and usually complete in under a second.

Click the button below to kick it off.
