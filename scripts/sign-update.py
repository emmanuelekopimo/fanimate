"""Sign release files for Fanimate's self-updater.

usage: python scripts/sign-update.py <file>...

Writes <file>.sig beside each file: the raw 64-byte Ed25519 signature of
the file's bytes. The key's 32-byte seed comes base64-encoded from the
UPDATE_SIGNING_KEY environment variable (a GitHub Actions secret); its
public half is `PUBLIC_KEY` in crates/fanimate-app/src/update.rs, and the
app refuses any update whose signature does not match it.

Needs the `cryptography` package.
"""

import base64
import os
import sys

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


def main() -> int:
    # a secret pasted or piped on Windows can carry a byte-order mark
    seed = os.environ.get("UPDATE_SIGNING_KEY", "").strip().lstrip("\ufeff").strip()
    if not seed:
        print("sign-update: UPDATE_SIGNING_KEY is not set; nothing signed", file=sys.stderr)
        return 1
    key = Ed25519PrivateKey.from_private_bytes(base64.b64decode(seed))
    for path in sys.argv[1:]:
        with open(path, "rb") as f:
            signature = key.sign(f.read())
        with open(path + ".sig", "wb") as f:
            f.write(signature)
        print(f"signed {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
