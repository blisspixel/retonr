"""Exercise folder selection and reload through a real retained terminal."""

import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


def exercise(binary, interrupt):
    with tempfile.TemporaryDirectory() as temporary:
        source = Path(temporary)
        (source / "a.txt").write_text("AAAAAAAAAAAA\n", encoding="utf-8")
        selected = source / "b.txt"
        selected.write_text("BBBBBBBBBBBB\n", encoding="utf-8")
        master, slave = pty.openpty()
        try:
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
            original = termios.tcgetattr(slave)
            environment = dict(os.environ, TERM="xterm-256color")
            child = subprocess.Popen(
                [binary, "tui", str(source), "--recursive"],
                stdin=slave, stdout=slave, stderr=slave, env=environment,
            )
            output = bytearray()

            def wait_for(text):
                deadline = time.monotonic() + 10
                while text not in output:
                    assert child.poll() is None, "review exited before expected preview"
                    assert time.monotonic() < deadline, "bounded terminal preview deadline"
                    if select.select([master], [], [], 0.1)[0]:
                        output.extend(os.read(master, 65536))

            try:
                wait_for(b"AAAAAAAAAAAA")
                output.clear()
                os.write(master, b"]")
                wait_for(b"BBBBBBBBBBBB")
                selected.write_text("CCCCCCCCCCCC\n", encoding="utf-8")
                output.clear()
                os.write(master, b"r")
                wait_for(b"CCCCCCCCCCCC")
                os.write(master, b"\x03" if interrupt else b"q")
                assert child.wait(timeout=10) == (130 if interrupt else 0)
                while select.select([master], [], [], 0)[0]:
                    output.extend(os.read(master, 65536))
                assert termios.tcgetattr(slave) == original, "terminal settings restored"
                assert b"\x1b[?1049l" in output, "alternate screen restored"
                assert b"\x1b[?25h" in output, "cursor restored"
                assert selected.read_bytes() == b"CCCCCCCCCCCC\n", "review wrote no document"
                assert sorted(path.name for path in source.iterdir()) == ["a.txt", "b.txt"]
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=5)
        finally:
            os.close(master)
            os.close(slave)


if __name__ == "__main__":
    exercise(sys.argv[1], False)
    exercise(sys.argv[1], True)
