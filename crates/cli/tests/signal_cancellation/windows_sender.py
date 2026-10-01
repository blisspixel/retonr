"""Send one targeted native Windows console event without runtime compilation."""

import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import sys
import time


def checked(function, *arguments):
    if not function(*arguments):
        raise ctypes.WinError(ctypes.get_last_error())


def main():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.FreeConsole.argtypes = []
    kernel.FreeConsole.restype = wintypes.BOOL
    kernel.AttachConsole.argtypes = [wintypes.DWORD]
    kernel.AttachConsole.restype = wintypes.BOOL
    kernel.SetConsoleCtrlHandler.argtypes = [ctypes.c_void_p, wintypes.BOOL]
    kernel.SetConsoleCtrlHandler.restype = wintypes.BOOL
    kernel.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.GenerateConsoleCtrlEvent.restype = wintypes.BOOL

    ready = Path(os.environ["RETONR_SIGNAL_READY"])
    request = Path(os.environ["RETONR_SIGNAL_REQUEST"])
    ready.write_text("ready", encoding="ascii")
    deadline = time.monotonic() + 30
    while not request.is_file():
        if time.monotonic() >= deadline:
            raise TimeoutError("Windows interrupt target was not published")
        time.sleep(0.005)
    target = int(request.read_text(encoding="ascii"))
    if not 0 < target <= 0xFFFFFFFF:
        raise ValueError("invalid Windows interrupt target")

    # A sender may start without a console; detaching that state is harmless.
    kernel.FreeConsole()
    checked(kernel.AttachConsole, target)
    checked(kernel.SetConsoleCtrlHandler, None, True)
    # CTRL_BREAK_EVENT can target the child's own process group.
    checked(kernel.GenerateConsoleCtrlEvent, 1, target)
    time.sleep(0.1)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            "Windows interrupt sender failed: "
            f"{type(error).__name__}; winerror={getattr(error, 'winerror', None)}",
            file=sys.stderr,
            flush=True,
        )
        sys.exit(1)
