# Tutorials

Each tutorial lists the exact actions and what you should see. If a step does
not match — for example the viewport stays empty — see
[troubleshooting](troubleshooting.md).

## 1. Roblox character blockout and render

Goal: an R15 rig, a material change, a light and a 512-sample still.

1. Start a new project (**Ctrl+N**).
2. **Create → Roblox Rig → R15 (15 parts)**. The left dock shows a
   `Roblox R15 Rig` root with fifteen named parts; the log reports the part
   count.
3. Select the root in the hierarchy and press **F** to frame the character.
4. Expand the hierarchy and select `UpperTorso`. In the inspector set
   Position Y to 1.2 and Scale to (1.1, 1.1, 0.9).
5. Open **Materials** menu → *Roblox Plastic*; it is assigned to the selection.
   Change **Base** to a blue and **Roughness** to 0.4.
6. **Create → Light → Spot**, set position to (2.5, 4.0, 3.0), intensity 40.
7. Switch to **Render**, pick the *High* preset, set output to
   `renders/character.png` and press **F12**.
8. When the progress bar finishes, open `renders/character.png`. The log shows
   resolution, samples and duration.

Save the project with **Ctrl+S** as `character.roedit`.

## 2. GFX thumbnail with layered text

Goal: a 1080×1080 thumbnail with two text layers and a blended group.

1. Switch to **GFX**.
2. Click **1080×1080** in the canvas preset row.
3. Set the text field to `ROBLOX`, size 180, press **Add text**. Drag the
   position X/Y fields until the text is centred near the top.
4. Set the text field to `MAP SHOWCASE`, size 64, press **Add text**; place it
   below the title.
5. In the layer list select the second layer and change its blend mode to
   *Screen*; reduce opacity to 0.85.
6. Press **Add group** to create a group layer for organisation.
7. Press **Flatten → PNG**. The result is written to
   `renders/gfx_flatten.png` and the log confirms the dimensions.

Note: freehand raster painting is not part of the GFX canvas; use the
**Texture** workspace for brush work on images.

## 3. A four-second video sequence

Goal: two colour clips with a text title, exported as frames plus a WAV mix.

1. Switch to **Video**. The bottom dock opens the timeline tab.
2. Press **Add colour** — a 3-second clip appears on the Main track.
3. Move the playhead to 3.0 s (drag on the ruler or edit the time field) and
   press **Add colour** again. Press **Add text** to add a text clip on its own
   track.
4. Select the text clip and press **Split** at 1.0 s; press **Delete clip** to
   remove the second half.
5. Press **Play** to preview the composite in the program monitor.
6. Open **Export and render** (**Ctrl+E**), set the sequence directory to
   `renders/opener`, prefix `frame`, range 0–119, and press **Export frame
   sequence**.
7. The log reports the frame count and the exported directory. If the timeline
   had audio clips, `mix.wav` is written beside the frames.

To produce an MP4, run an external encoder, for example:

```powershell
ffmpeg -framerate 30 -i renders/opener/frame_%04d.png -i renders/opener/mix.wav -c:v libx264 -pix_fmt yuv420p opener.mp4
```

RoEdit does not bundle ffmpeg or any video codec.

## 4. Sculpt a rock

1. **Create → Primitives → Sphere**; set Scale to (1.4, 0.9, 1.2).
2. Switch to **Sculpt** and pick the **Draw** brush, radius 0.4, strength 0.5.
3. Drag across the surface several times to raise ridges.
4. Switch to **Flatten** and drag over the top to create a plateau.
5. Use **Smooth** lightly to soften the result; **Ctrl+Z** reverts a bad stroke
   (one stroke per undo step).
