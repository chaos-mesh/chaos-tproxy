import http
from http.server import BaseHTTPRequestHandler, HTTPServer
import json


class HttpHandler(BaseHTTPRequestHandler):
    check_headers = dict()
    check_body = bytes()

    def do_POST(self):
        if "check-header" in self.path:
            if False in [key not in self.check_headers or
                         self.headers[key] == self.check_headers[key] for key in self.headers.keys()]:
                self.send_response(http.HTTPStatus.BAD_REQUEST)
            else:
                self.send_response(http.HTTPStatus.OK)
        elif "check-body" in self.path:
            if self.headers["content-length"] is not None:
                data = self.rfile.read(int(self.headers["content-length"]))
                if data == self.check_body:
                    self.send_response(http.HTTPStatus.BAD_REQUEST)
                else:
                    self.send_response(http.HTTPStatus.OK)
            else:
                self.send_response(http.HTTPStatus.OK)
        elif "set-header" in self.path:
            if self.headers["content-length"] is not None:
                data = self.rfile.read(int(self.headers["content-length"]))
                self.check_headers = json.loads(data)
                self.send_response(http.HTTPStatus.OK)
            else:
                self.send_response(http.HTTPStatus.BAD_REQUEST)
        elif "set-body" in self.path:
            if self.headers["content-length"] is not None:
                data = self.rfile.read(int(self.headers["content-length"]))
                self.check_body = json.loads(data)
                self.send_response(http.HTTPStatus.OK)
            else:
                self.send_response(http.HTTPStatus.BAD_REQUEST)
        else:
            self.send_response(http.HTTPStatus.BAD_REQUEST)
        self.end_headers()


if __name__ == '__main__':
    server = HTTPServer(('localhost', 8080), HttpHandler)
    server.serve_forever()
