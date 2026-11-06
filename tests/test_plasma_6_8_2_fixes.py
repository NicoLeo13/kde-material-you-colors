import argparse
import logging
import pathlib
import re
import unittest
from unittest.mock import MagicMock, patch

import kde_material_you_colors.settings  # noqa: F401
from kde_material_you_colors.utils import utils, wallpaper_utils

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
QML_DIR = REPO_ROOT / "src" / "plasmoid" / "package" / "contents" / "ui"


class TestPlasma682Fixes(unittest.TestCase):
    def test_wide_argparse_help_skips_stty_when_not_tty(self):
        with patch("sys.stdin") as mock_stdin, patch("os.popen") as mock_popen:
            mock_stdin.isatty.return_value = False
            utils.wide_argparse_help(argparse.HelpFormatter)
            mock_popen.assert_not_called()

    def test_get_wallpaper_config_empty_dict_logs_warning_without_exception_traceback(self):
        mock_bus = MagicMock()
        mock_iface = MagicMock()
        mock_iface.wallpaper.return_value = {}
        with (
            patch.object(wallpaper_utils.dbus, "SessionBus", return_value=mock_bus),
            patch.object(wallpaper_utils.dbus, "Interface", return_value=mock_iface),
            patch.object(logging, "exception") as mock_log_exc,
            patch.object(logging, "warning") as mock_log_warn,
        ):
            with self.assertRaises(ValueError):
                wallpaper_utils.get_wallpaper_config(0)
            mock_log_exc.assert_not_called()
            mock_log_warn.assert_called_once()

    def test_qml_full_representation_declares_toolbar_opacity_override_and_guards_colors(self):
        full_rep = (QML_DIR / "FullRepresentation.qml").read_text(encoding="utf-8")
        self.assertIn("property bool toolbar_opacity_override:", full_rep)
        self.assertNotIn("anchors.centerIn: parent\n                }", full_rep)
        self.assertNotIn("anchors.left: headingTextColors.right", full_rep)
        self.assertNotIn("anchors.left: headingDarkMode.right", full_rep)
        self.assertNotIn('color: colorList.split(" ")[index]\n', full_rep)
        self.assertNotIn("ps -o user:256,pid,cmd -C ", full_rep)

    def test_qml_colors_preview_guards_null_mdc_and_scheme(self):
        colors_preview = (QML_DIR / "components" / "ColorsPreview.qml").read_text(encoding="utf-8")
        role_preview = (QML_DIR / "components" / "ColorRolePreview.qml").read_text(encoding="utf-8")
        self.assertNotIn("property var scheme: r.mdc.schemes[r.theme]", colors_preview)
        self.assertNotIn("Object.keys(r.mdc.custom)", colors_preview)
        self.assertNotIn("color: scheme[root.role]\n", role_preview)


if __name__ == "__main__":
    unittest.main()
