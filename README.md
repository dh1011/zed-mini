# Zed Mini

Zed Mini is an unofficial modified fork of [Zed](https://github.com/zed-industries/zed), focused on local text editing.

The original Zed project is developed by Zed Industries, Inc. This fork is not affiliated with or endorsed by Zed Industries.

## Licensing

Zed Mini is based on Zed source code, which is licensed primarily under GPL-3.0-or-later, with Apache-2.0 components where marked. See [LICENSE-GPL](LICENSE-GPL) and [LICENSE-APACHE](LICENSE-APACHE).

This repository contains modified source code. Binary distributions should include the corresponding source code, these license files, and dependency license attributions.

## Development tips

### Dependencies

* Install [Postgres.app](https://postgresapp.com) and start it.
* Install the `LiveKit` server and the `foreman` process supervisor:

    ```
    brew install livekit
    brew install foreman
    ```

* Ensure the Zed.dev website is checked out in a sibling directory:

    ```
    cd ..
    git clone https://github.com/zed-industries/zed.dev
    ```

* Initialize submodules

    ```
    git submodule update --init --recursive
    ```

* Set up a local `zed` database and seed it with some initial users:

    Create a personal GitHub token to run `script/bootstrap` once successfully. Then delete that token.

    ```
    GITHUB_TOKEN=<$token> script/bootstrap
    ```

### Testing against locally-running servers

Start the web and collab servers:

```
foreman start
```

If you want to run Zed pointed at the local servers, you can run:

```
script/zed-with-local-servers
# or...
script/zed-with-local-servers --release
```

### Dump element JSON

If you trigger `cmd-alt-i`, Zed will copy a JSON representation of the current window contents to the clipboard. You can paste this in a tool like [DJSON](https://chrome.google.com/webstore/detail/djson-json-viewer-formatt/chaeijjekipecdajnijdldjjipaegdjc?hl=en) to navigate the state of on-screen elements in a structured way.

### Dependency Licensing

We use [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) to automatically comply with open source licenses. If CI is failing, check the following:

- Is it showing a `no license specified` error for a crate you've created? If so, add `publish = false` under `[package]` in your crate's Cargo.toml.
- Is the error `failed to satisfy license requirements` for a dependency? If so, first determine what license the project has and whether this system is sufficient to comply with this license's requirements. If you're unsure, ask a lawyer. Once you've verified that this system is acceptable add the license's SPDX identifier to the `accepted` array in `script/licenses/zed-licenses.toml`.
- Is `cargo-about` unable to find the license for a dependency? If so, add a clarification field at the end of `script/licenses/zed-licenses.toml`, as specified in the [cargo-about book](https://embarkstudios.github.io/cargo-about/cli/generate/config.html#crate-configuration).

