export function riceWallpaperModeArgument(arguments_) {
  return (
    arguments_
      .find((argument) => argument.startsWith("--wallpaper-mode="))
      ?.slice("--wallpaper-mode=".length) || null
  );
}
