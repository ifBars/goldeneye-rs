"""Serve public progress summaries, recomputing freshness before each request."""
from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
FILES = {"/": "index.html", "/index.html": "index.html", "/latest.json": "latest.json",
         "/history.json": "history.json", "/roadmap.json": "roadmap.json"}


def handler(progress: Path, refresh):
    class ProgressHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            self.reply(False)

        def do_HEAD(self):
            self.reply(True)

        def reply(self, head: bool):
            name = FILES.get(unquote(urlsplit(self.path).path))
            if name is None:
                self.send_error(404)
                return
            try:
                refresh()
                path = progress / name
                if progress.is_symlink() or path.is_symlink() or path.resolve().parent != progress.resolve():
                    raise ValueError("summary path escaped progress directory")
                payload = path.read_bytes()
            except Exception as error:
                # Never serve the previous verified count when refresh fails.
                print(f"Progress refresh failed: {error}", file=sys.stderr, flush=True)
                self.send_error(503, "Progress refresh failed; no current result is available")
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8" if name.endswith(".html")
                             else "application/json; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            if not head:
                self.wfile.write(payload)

    return ProgressHandler


class ProgressServer(HTTPServer):
    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(10)
        return connection, address


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8080)
    args = parser.parse_args()
    tool = ROOT / "target" / "debug" / ("xtask.exe" if sys.platform == "win32" else "xtask")

    def refresh():
        if not tool.is_file():
            raise RuntimeError("run cargo xtask report once to build the report tool")
        result = subprocess.run([str(tool), "report"], cwd=ROOT, capture_output=True,
                                text=True, timeout=30, check=False)
        if result.returncode:
            print(result.stdout + result.stderr, file=sys.stderr, flush=True)
            raise RuntimeError("report generator rejected the current evidence")

    server = ProgressServer((args.bind, args.port), handler(ROOT / "progress", refresh))
    print(f"Live progress: http://{args.bind}:{args.port}/", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
