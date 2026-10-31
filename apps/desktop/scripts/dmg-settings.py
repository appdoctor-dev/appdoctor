# dmgbuild settings for AppDoctor-macOS.dmg: AppDoctor.app beside a link to
# Applications, on the background in assets/dmg, in a 660 × 420 window with no
# toolbar or sidebar. scripts/build-dmg.py passes it to dmgbuild, with
# `app` (the .app to include) and `assets` (apps/desktop/assets) defined.
#
# dmgbuild writes the window layout itself, so this works without Finder
# (on CI too). The icon centres match the glows in assets/dmg/background.html.
import os.path

app = defines["app"]  # noqa: F821  (dmgbuild provides `defines`)
assets = defines["assets"]  # noqa: F821

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = os.path.join(assets, "app-icon", "AppIcon.icns")

# A background.png with a background@2x.png beside it becomes one Retina TIFF.
background = os.path.join(assets, "dmg", "background.png")
# 420 points of background plus Finder's title bar.
window_rect = ((200, 160), (660, 452))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False
icon_size = 128
text_size = 13
icon_locations = {
    os.path.basename(app): (165, 196),
    "Applications": (495, 196),
}
