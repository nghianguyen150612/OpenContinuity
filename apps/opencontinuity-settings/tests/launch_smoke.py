"""Start the installed native executable; assert it stays open without QML errors."""
import subprocess
import sys

process = subprocess.Popen([sys.argv[1]], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    stdout, stderr = process.communicate(timeout=2)
except subprocess.TimeoutExpired:
    process.terminate()
    stdout, stderr = process.communicate(timeout=5)
    if any(marker in stderr for marker in (b"failed to load", b"ReferenceError", b"TypeError", b"is not installed", b"Binding loop")):
        raise SystemExit(stderr.decode(errors="replace"))
    print("Installed native application remained open; no QML runtime errors.")
else:
    raise SystemExit(f"Application exited before smoke deadline ({process.returncode}): {stderr!r}")
