# Assets

`logo.png` is the master artwork for the app icon. Everything in
`src-tauri/icons/` is generated from it, so don't edit those by hand.

To regenerate the icon set after changing the logo:

```bash
python3 scripts/generate-icons.py
```

A larger or vector source would be better: the current master is 375×375, so
the 512px icon is a slight upscale.

## Screenshots

`screenshots/` holds the images used in the top-level README. All three are the
app window at 785x740, captured in the dark theme; keep new ones consistent with
that so the set does not look assembled from different machines.

The recording shown in them is a Yale open course lecture, deliberately: a
screenshot of real work would publish whatever was said in it.
