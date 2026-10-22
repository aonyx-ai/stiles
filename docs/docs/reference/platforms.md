# Supported platforms

<!--
goal: tell a reader whether their machine has a prebuilt archive.
non-goal: install instructions. install.md owns those.
-->

Whisker supports the following platforms:

| Platform               | Target                      |
| ---------------------- | --------------------------- |
| Linux on x86-64        | `x86_64-unknown-linux-gnu`  |
| Linux on arm64         | `aarch64-unknown-linux-gnu` |
| macOS on Apple silicon | `aarch64-apple-darwin`      |

On Linux we require glibc version 2.35 or higher. This is supported by Ubuntu
22.04 and Debian 12.

If your platform is not listed, you still may be able to build from source.
