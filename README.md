# Maplewood

The humble beginnings of a 2D adventure game engine built from the ground up with 🦀 Rust and incorporating scripting with 🌙 Lua. This is intended to be the foundation of a long-term personal hobby game project.

[Video Demo](https://www.youtube.com/watch?v=OA7HsZCEViI)

## Basic Features

- ⚙️ A simple Entity-Component-System architecture.  
  Entities are containers of optional Components, which are containers of related data. Unlike in a full ECS, there is no first-class System abstraction or multi-threading; Components are queried and operated on in sequential, inline code.

- 📝 Concurrent event scripting with Lua.  
  Scripts have various triggers (such as entity collision, interaction, or automatic given a condition). Scripts provide the functions for scripting much of the story and game content (such as displaying messages, animating entities, playing sound effects and music, setting and branching on story variables, etc).

- 🗺️ Simple 2D graphics and a tile-based map.  
  *(The next focus of development is the game world/map.)*

## Notable Libraries Used

[SDL2](https://www.libsdl.org/) (via Rust bindings from [Rust-SDL2](https://github.com/Rust-SDL2/rust-sdl2)) provides cross-platform, low-level access to input, audio, and graphics hardware.

[rlua](https://github.com/amethyst/rlua) provides a high-level, Rust-y interface to the Lua C API.

## Building and Running

[Install Rust](https://www.rust-lang.org/tools/install) using rustup. Select the nightly toolchain. For Windows, install the Visual Studio build tools if prompted. The minimal requirements are:

- `MSVC v143 - VS 2022 C++ x64/x86 build tools (Latest)`
- `Windows 11 SDK (10.0.22621.0)`

Build and run with `cargo run` in the project root directory.

### Requires SDL Development Libraries

[SDL2](https://github.com/libsdl-org/SDL/releases) | [SDL2_image](https://github.com/libsdl-org/SDL_image/releases) |
[SDL2_ttf](https://github.com/libsdl-org/SDL_ttf/releases) |
[SLD2_mixer](https://github.com/libsdl-org/SDL_mixer/releases)

For Windows, download `SDL2-devel-2.x.x-VC.zip` from the latest releases, copy all `.lib` files to `C:\Users\{username}\.rustup\toolchains\{toolchain}\lib\rustlib\{toolchain}\lib`, and copy all `.dll` files to the project root directory.

For Ubuntu, run `sudo apt-get install libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev libsdl2-mixer-dev`.

*Requires nightly Rust with version no later than nightly-2024-08-03. (PR #128370 in nightly-2024-08-04 breaks SDL lib linking for some reason).*
