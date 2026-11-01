import fcntl
import json
import os
from pathlib import Path
import pty
import signal
import subprocess
import sys
import termios
import unittest


def without_pending_input_flag(attributes):
    attributes = attributes.copy()
    attributes[3] &= ~getattr(termios, "PENDIN", 0)
    return attributes


def probe(binary):
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    before = termios.tcgetattr(slave)
    read_fd, write_fd = os.pipe()
    os.close(read_fd)
    try:
        result = subprocess.run(
            [binary, "--mute", "--no-intro", "--halfblock"],
            stdin=slave,
            stdout=write_fd,
            stderr=subprocess.PIPE,
            timeout=10,
        )
        after = termios.tcgetattr(slave)
        print(json.dumps({
            "exit_code": result.returncode,
            "error": result.stderr.decode(),
            "restored": without_pending_input_flag(before) == without_pending_input_flag(after),
            "canonical": bool(after[3] & termios.ICANON),
            "echo": bool(after[3] & termios.ECHO),
        }))
    finally:
        termios.tcsetattr(slave, termios.TCSANOW, before)
        for descriptor in [master, slave, write_fd]:
            os.close(descriptor)


class TerminalCleanupTests(unittest.TestCase):
    def test_startup_output_failure_restores_terminal(self):
        binary = Path(__file__).resolve().parents[1] / "target/debug/slopracer-f1"
        result = subprocess.run(
            [sys.executable, __file__, "--probe", str(binary)],
            capture_output=True,
            text=True,
            start_new_session=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        state = json.loads(result.stdout)
        self.assertNotEqual(state["exit_code"], 0)
        self.assertIn("BrokenPipe", state["error"])
        self.assertTrue(state["restored"], state)
        self.assertTrue(state["canonical"], state)
        self.assertTrue(state["echo"], state)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--probe":
        probe(sys.argv[2])
    else:
        unittest.main()
