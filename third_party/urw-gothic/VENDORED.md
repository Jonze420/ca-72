# Vendored: URW Gothic

The panel's font (decisions.md R22), so the panel is drawn with the same letters on every
platform. Not modified.

| File | Licence | Copied from |
|---|---|---|
| `URWGothic-Book.otf`, `URWGothic-Demi.otf` | GNU Affero General Public License 3.0, with a font exception (`LICENSE`) | [ArtifexSoftware/urw-base35-fonts](https://github.com/ArtifexSoftware/urw-base35-fonts), tag `20200910`, `fonts/` |
| `COPYING`, `LICENSE`, `README.md` | (the licence, its exception and the collection's readme) | the same tag, its root |

URW Gothic, by (URW)++, is one of the URW base 35 fonts; TeX Gyre Adventor, the panel's
font before R22, was made from it. The panel uses Book and Demi (its bold); the obliques are
left out.

SHA-256 of the fonts: `URWGothic-Book.otf`
04318316cee29950805110c9c8949eea189ef5575132881f4d4d7e03e5299903, `URWGothic-Demi.otf`
5b009410cf5231dcb1e45b155c1afedcfc63d82042fd8c414d0dd7705c9fbbae. SHA-256 of the tree (every
file but this one, sorted by path, as
`find . -type f ! -name VENDORED.md -print0 | sort -z | xargs -0 sha256sum | sha256sum`
from this directory): `a7038a5a595253d5ae131ddd8a3529fc6a9509c9697eebb7cf097d7738c95c1d`.

To update: download the same files from a newer tag, run `cargo test -p ca72-panel` (the
panel as drawn against the approved design), and record the tag and hashes here.
