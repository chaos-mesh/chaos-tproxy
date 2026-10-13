from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"network-ok\n"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("X-Tproxy-Test", self.headers.get("X-Tproxy-Test", "direct"))
        self.end_headers()
        self.wfile.write(body)


HTTPServer(("0.0.0.0", 18080), Handler).serve_forever()
