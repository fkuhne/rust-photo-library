# Nikon's focus point maps

Where each AF point of a Nikon camera lies in its image, for `af::nikon` (record [0022](../../spec/decisions/0022-canon-s-and-nikon-s-focus-points-from-their-af-blocks-nikon-s-named-points-by-the-focus-points-plugin-s-maps.md)): one file per camera, each point's middle in the camera's own image pixels, `A1 = {x, y}` or `{x, y, width, height}`, the image's size in a comment at the top, and `focusPointDimens` the points' size where they're all one.

They come unchanged from the Focus Points plugin for Lightroom Classic, <https://github.com/musselwhizzle/Focus-Points>, `focuspoints.lrplugin/focus_points/nikon/`, at commit `9e008821b9dd95c7c2b5bae655a29ed282fbf0d1`, taken on 2026-11-26, under the Apache License 2.0, in [`LICENSE`](LICENSE). The plugin has no NOTICE file. Which camera shares which map is from its `NikonDuplicates.lua`, written down in `af.rs`.
