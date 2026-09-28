"""Build configuration for Kaji's platform-native Python wheels."""

import os
from pathlib import Path

from setuptools import find_packages, setup
from wheel.bdist_wheel import bdist_wheel as _bdist_wheel


PLATFORM_TAGS = {
    "darwin-arm64": ("py3", "none", "macosx_11_0_arm64"),
    "darwin-x64": ("py3", "none", "macosx_10_13_x86_64"),
    "linux-x64-gnu": ("py3", "none", "manylinux_2_35_x86_64"),
    "win32-x64-msvc": ("py3", "none", "win_amd64"),
}


class bdist_wheel(_bdist_wheel):
    """Tag a data-only wheel for the native executables embedded in it."""

    def finalize_options(self):
        super().finalize_options()
        # The Python code is pure, but this wheel carries platform-native binaries.
        self.root_is_pure = False

    def get_tag(self):
        platform = os.environ.get("KAJI_PYTHON_PLATFORM")
        if platform not in PLATFORM_TAGS:
            choices = ", ".join(PLATFORM_TAGS)
            raise RuntimeError(
                "KAJI_PYTHON_PLATFORM must identify the packaged native binaries "
                f"({choices}); got {platform!r}"
            )
        return PLATFORM_TAGS[platform]


root = Path(__file__).parent

setup(
    name="kaji-cli",
    version="0.2.1",
    description="Native Kaji OpenAPI SDK generator for Python environments",
    long_description=(root / "README.md").read_text(encoding="utf-8"),
    long_description_content_type="text/markdown",
    python_requires=">=3.9",
    license="MIT",
    author="Relevate",
    url="https://github.com/rlvtapp/kaji",
    project_urls={
        "Documentation": "https://github.com/rlvtapp/kaji/tree/main/docs",
        "Source": "https://github.com/rlvtapp/kaji",
        "Issues": "https://github.com/rlvtapp/kaji/issues",
    },
    keywords="openapi swagger sdk codegen generator",
    classifiers=[
        "Development Status :: 4 - Beta",
        "Environment :: Console",
        "License :: OSI Approved :: MIT License",
        "Operating System :: MacOS",
        "Operating System :: Microsoft :: Windows",
        "Operating System :: POSIX :: Linux",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3 :: Only",
        "Topic :: Software Development :: Code Generators",
    ],
    package_dir={"": "src"},
    packages=find_packages("src"),
    package_data={"kaji_cli": ["bin/*"]},
    include_package_data=True,
    entry_points={"console_scripts": ["kaji=kaji_cli:main"]},
    cmdclass={"bdist_wheel": bdist_wheel},
)
