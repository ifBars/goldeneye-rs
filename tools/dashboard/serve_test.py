import importlib.util
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import urlopen

spec = importlib.util.spec_from_file_location("progress_serve", Path(__file__).with_name("serve.py"))
serve = importlib.util.module_from_spec(spec)
spec.loader.exec_module(serve)


class LiveProgressTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.state = "verified 1"
        self.refreshes = 0

        def refresh():
            self.refreshes += 1
            if self.state == "error":
                raise RuntimeError("authored refresh failure")
            (self.root / "index.html").write_text(self.state, encoding="utf-8")

        self.server = serve.ProgressServer(("127.0.0.1", 0), serve.handler(self.root, refresh))
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.01})
        self.thread.start()
        self.url = f"http://127.0.0.1:{self.server.server_port}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)
        self.directory.cleanup()

    def test_each_refresh_replaces_stale_counts_and_disables_browser_cache(self):
        with urlopen(self.url, timeout=5) as response:
            self.assertEqual(response.read(), b"verified 1")
            self.assertEqual(response.headers["Cache-Control"], "no-store")
        self.state = "verified 0 stale"
        with urlopen(self.url, timeout=5) as response:
            self.assertEqual(response.read(), b"verified 0 stale")
        self.assertEqual(self.refreshes, 2)

    def test_failed_refresh_cannot_serve_previous_verified_html(self):
        with urlopen(self.url, timeout=5) as response:
            self.assertIn(b"verified 1", response.read())
        self.state = "error"
        with self.assertRaises(HTTPError) as caught:
            urlopen(self.url, timeout=5)
        self.assertEqual(caught.exception.code, 503)
        self.assertNotIn(b"verified 1", caught.exception.read())

    def test_private_paths_and_directory_listing_are_not_served(self):
        for path in ["/../local/secret", "/%2e%2e/local/secret", "/unknown", "/local/"]:
            with self.assertRaises(HTTPError) as caught:
                urlopen(self.url + path, timeout=5)
            self.assertEqual(caught.exception.code, 404)
        self.assertEqual(self.refreshes, 0)


if __name__ == "__main__":
    unittest.main()
