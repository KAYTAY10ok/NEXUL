"""Serve the repository's web example on loopback, with portable module MIME types."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

class Handler(SimpleHTTPRequestHandler):
    # Windows registry MIME mappings may otherwise classify .mjs as text/plain.
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map,
                      '.mjs': 'text/javascript', '.wasm': 'application/wasm'}

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8766)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    handler = partial(Handler, directory=str(root))
    print(f'http://127.0.0.1:{args.port}/examples/web/', flush=True)
    with ThreadingHTTPServer(('127.0.0.1', args.port), handler) as server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
