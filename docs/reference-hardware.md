# Reference hardware

The performance budget (60 fps for a 40-bone rig with 10 Smart Bones, project open under 3 s, no UI action over 100 ms) is measured on a "mid-range laptop". This page defines that machine so the budget can pass or fail.

> **Status: proposal.** Written at the end of milestone M1 for the maintainers to confirm or change. Until confirmed, benchmark numbers from any machine are welcome, but none of them pass or fail Gate 1.

## The reference machines

| Platform | Reference | Why this one |
|---|---|---|
| Windows | Laptop with an Intel Core i5-1235U (12th generation), Intel Iris Xe graphics (80 EU), 8 GB dual-channel RAM, SSD, 1920×1080 screen at 125% scaling, Windows 11 | The most common mid-range laptop configuration sold from 2022 to 2024; integrated graphics, like most of the target audience's machines |
| Linux | The same laptop, Ubuntu 24.04 LTS with its stock Mesa driver | Keeps the hardware constant so Windows and Linux numbers compare directly |
| macOS | MacBook Air M1 (2020), 8 GB, 7-core GPU, macOS 13 or later | The cheapest and oldest Apple silicon Mac still in wide use, and the oldest macOS version Fanimate supports |

A machine with an AMD Ryzen 5 7530U (Radeon Vega 7 graphics) is close enough to the Windows reference to stand in for it; note it in the results.

## The floor

Below the reference, Fanimate should still run and be usable, with no budget promise. The floor machine is the one M1 was developed on: a laptop with an Intel Core i5-6200U (2015) and its **Intel HD Graphics 520**, 8 GB RAM, Windows 10. It is also the machine whose Vulkan driver crashes on load and whose OpenGL driver crashes intermittently on exit, which is why Windows uses DirectX 12 by default.

## How to measure

Use a release build, plugged in, with the power plan on balanced and nothing else heavy running.

```sh
cargo run --release -p fanimate-app -- --bench --no-vsync      # in the app, UI included, 1280x800 or larger window
cargo run --release -p fanimate-cli -- bench --gpu             # no window: CPU breakdown, then offscreen GPU render
cargo run --release -p fanimate-cli -- bench --gpu --bones 400 # stress scene
```

`fanimate-cli bench --gpu` waits for the GPU after every frame, so its frame rate is lower than the app's on the same machine; it is the figure to compare between machines and releases. Post the full output with the CPU, GPU, operating system, screen resolution and scaling.

## Results so far

| Machine | Scene | App (`--bench --no-vsync`) | CLI GPU render | CPU per frame |
|---|---|---|---|---|
| Floor: HD Graphics 520, Windows 10, DirectX 12 | 40 bones, 81 shapes | 165 to 183 fps | 123 fps | 0.4 to 0.6 ms |
| Floor: HD Graphics 520, Windows 10, DirectX 12 | 400 bones, 801 shapes | 156 fps | 84 fps | 2.2 to 3.5 ms |
| Windows reference | | not measured | not measured | |
| Linux reference | | not measured | not measured | |
| macOS reference | | not measured | not measured | |

Every release build also runs `fanimate-cli bench --gpu` on the Windows, macOS and Linux build machines and puts the output in the run summary. Those machines have no real GPU (a software renderer or a virtual GPU), so their numbers show that rendering works on each platform, not that the budget holds.
