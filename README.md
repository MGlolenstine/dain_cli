# Dain CLI
A pretty basic and simple CLI interface for both DAIN and RIFE algorithms.

Programs that allow these algorithms to run can be found here:
- [RIFE](https://github.com/nihui/rife-ncnn-vulkan)
- [DAIN](https://github.com/nihui/dain-ncnn-vulkan)

(massive thanks to [nihui](https://github.com/nihui) for providing pre-built binaries and awesome work on cross-platform support)

# Prerequisites
Before you run it, you'll need to have [`ffmpeg`](https://ffmpeg.org/) installed.
On linux and macOS it's as easy as running a command, but for windows you'll have to install it manually.

## Windows
1. Download `ffmpeg-*-win64-lgpl-*.zip` from [here](https://github.com/BtbN/FFmpeg-Builds/releases).
2. Extract it into a folder somewhere.
3. Open the start menu and start typing `Environment variables`.
4. Open the program, and in the bottom half of the window double click on `Path`.
5. Click on the `Add` button on the right side of the screen.
6. Type in `ffmpeg` location path.
7. Click ok and restart your computer.


# Usage
If you run the command without arguments, it'll show the usage.

```
$ dain-cli
```
```
dain-cli <input_video> <output_video> <framework> [<target_framerate>]
framework can be either `rife` or `dain`
Rife: Fast framework, but it can only double the framerate
DAIN: Very slow model, but it can set custom framerate
target_framerate: Only respected in DAIN, RIFE only does 2x on current framerate.
If not specified for DAIN, it defaults to 60.0
```

The command is split into 4 arguments.
### `input_video`
Input video contains a path that's poiting towards the video you want to enhance.

### `output_video`
Output video contains a path that's poiting towards the enhanced video destination.

### `framework`
Framework currently has two different options to choose from:
- DAIN
- RIFE

Each of them has their own pros and cons:
### DAIN
- is really slow
- increases FPS with better interpolation
- can have custom FPS set
### RIFE
- much faster than DAIN
- always only increments FPS by 2

### `target_framerate`
Target framerate is currently only respected in DAIN, as RIFE always only doubles the FPS.

It's optional and if left out, will default to 60.

# Benchmarks
I've run few benchmarks on my system (r7 1800x and 5700XT; Arch Linux) on a 720p 10min video.

FRAMEWORK|RATIO|
---|---|
DAIN|75x|
RIFE|3.45x|

Ratio being `render_time/video_size`