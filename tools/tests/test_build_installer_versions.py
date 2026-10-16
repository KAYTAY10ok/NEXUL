import importlib.util
import json
import os
import unittest
from unittest.mock import patch


SCRIPT = os.path.join(os.path.dirname(os.path.dirname(__file__)), "build_installer.py")
SPEC = importlib.util.spec_from_file_location("build_installer", SCRIPT)
build_installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build_installer)


class PackageVersionsTests(unittest.TestCase):
    @patch.object(build_installer.subprocess, "check_output")
    def test_reads_workspace_package_names_and_versions(self, check_output):
        check_output.return_value = json.dumps({
            "packages": [
                {"name": "nxc", "version": "0.3.0"},
                {"name": "nexul-installer", "version": "0.3.0"},
            ]
        })

        result = build_installer.package_versions()

        self.assertEqual(result, {"nxc": "0.3.0", "nexul-installer": "0.3.0"})
        check_output.assert_called_once_with(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=build_installer.ROOT,
            text=True,
        )


class ReleaseVersionGuardTests(unittest.TestCase):
    def setUp(self):
        self.workspace = {
            "nxc": "0.3.0",
            "nexul-studio": "0.3.0",
            "nexul-installer": "0.3.0",
        }
        self.native = {"nexul-studio-app": "0.3.0"}

    @patch.object(build_installer, "package_versions")
    def test_accepts_aligned_release_packages(self, package_versions):
        package_versions.side_effect = [self.workspace, self.native]

        with patch("builtins.print") as print_mock:
            build_installer.check_release_versions()

        print_mock.assert_called_once_with("versions de distribution :", "0.3.0")
        self.assertEqual(package_versions.call_count, 2)
        self.assertEqual(
            package_versions.call_args_list[1].args,
            (os.path.join("studio", "app", "Cargo.toml"),),
        )

    @patch.object(build_installer, "package_versions")
    def test_rejects_mixed_versions_and_lists_each_component(self, package_versions):
        package_versions.side_effect = [self.workspace, {"nexul-studio-app": "0.2.0"}]

        with self.assertRaises(SystemExit) as error:
            build_installer.check_release_versions()

        self.assertIn("NEXUL Studio natif=0.2.0", str(error.exception))
        self.assertIn("installateur=0.3.0", str(error.exception))
        self.assertIn("Alignez les versions", str(error.exception))

    @patch.object(build_installer, "package_versions")
    def test_rejects_a_missing_release_package(self, package_versions):
        package_versions.side_effect = [self.workspace, {}]

        with self.assertRaises(SystemExit) as error:
            build_installer.check_release_versions()

        self.assertIn("Paquets Cargo introuvables", str(error.exception))
        self.assertIn("NEXUL Studio natif", str(error.exception))


if __name__ == "__main__":
    unittest.main()
