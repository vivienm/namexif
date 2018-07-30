from os.path import dirname, join

from setuptools import find_packages, setup


KEYWORDS = []
CLASSIFIERS = [
    "Intended Audience :: Developers",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.6",
    "Programming Language :: Python :: 3.7",
    "Programming Language :: Python :: Implementation :: CPython",
    "Programming Language :: Python",
    "Topic :: Multimedia :: Graphics",
    "Topic :: Utilities",
]
INSTALL_REQUIRES = ["click", "python-dateutil"]


PROJECT_DIR = dirname(__file__)
README_FILE = join(PROJECT_DIR, "README.rst")
ABOUT_FILE = join(PROJECT_DIR, "src", "namexif", "__about__.py")


def get_readme():
    with open(README_FILE) as fileobj:
        return fileobj.read()


def get_about():
    about = {}
    with open(ABOUT_FILE) as fileobj:
        exec(fileobj.read(), about)
    return about


ABOUT = get_about()


setup(
    name=ABOUT["__title__"],
    version=ABOUT["__version__"],
    description=ABOUT["__summary__"],
    long_description=get_readme(),
    author=ABOUT["__author__"],
    author_email=ABOUT["__email__"],
    url=ABOUT["__uri__"],
    keywords=KEYWORDS,
    classifiers=CLASSIFIERS,
    package_dir={"": "src"},
    packages=find_packages("src"),
    entry_points={"console_scripts": ["namexif=namexif.__main__:main"]},
    install_requires=INSTALL_REQUIRES,
    python_requires=">=3.6, <4",
    zip_safe=False,
)
