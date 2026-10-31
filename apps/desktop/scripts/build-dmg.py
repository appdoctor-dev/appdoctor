"""Builds AppDoctor-macOS.dmg with dmgbuild and scripts/dmg-settings.py.

    python3 build-dmg.py <AppDoctor.app> <out.dmg>

dmgbuild records the window background twice in .DS_Store: as an alias and
as a bookmark (pBBk). The bookmark doesn't survive mounting the image
elsewhere, and recent Finder versions trust it over the alias, so the window
opens without its background. Leaving the bookmark out (as other DMG
builders do) keeps the alias, which Finder resolves.
"""

import os
import sys

import dmgbuild
import dmgbuild.core


class _NoBookmark:
    @staticmethod
    def for_file(_path):
        return None


dmgbuild.core.Bookmark = _NoBookmark

app, out = sys.argv[1], sys.argv[2]
here = os.path.dirname(os.path.abspath(__file__))
dmgbuild.build_dmg(
    out,
    "AppDoctor",
    os.path.join(here, "dmg-settings.py"),
    defines={"app": app, "assets": os.path.join(here, "..", "assets")},
)
