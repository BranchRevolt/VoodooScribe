# Assets

`logo.png` is the master artwork for the app icon. Everything in
`src-tauri/icons/` is generated from it — don't edit those by hand.

To regenerate the icon set after changing the logo:

```bash
python3 scripts/generate-icons.py
```

A larger or vector source would be better: the current master is 375×375, so
the 512px icon is a slight upscale.
