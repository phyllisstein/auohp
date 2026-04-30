# `@auohp/desktop`: Local inference for knowledge graphs

## Rust layer
### Tauri UI
Creating the tray icon:

```shell
magick icon.png -resize 32x32 -alpha off -colorspace gray -negate alpha.png
magick -size 32x32 canvas:black alpha.png -compose CopyOpacity -composite PNG32:32x32.png
```
