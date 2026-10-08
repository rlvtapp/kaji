import http.server, pathlib, sys, threading, time
root=pathlib.Path(sys.argv[1])
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def do_GET(self):
        assert self.headers.get('Authorization') == 'Bearer test'
        if self.path.startswith('/redirect'):
            self.send_response(302);self.send_header('Location','https://unused.invalid/events');self.end_headers();return
        self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
        try:
            if self.path.startswith('/cancel'):
                self.wfile.write(b'data: waiting\n\n');self.wfile.flush()
                for _ in range(500):
                    time.sleep(.01);self.wfile.write(b':heartbeat\n\n');self.wfile.flush()
                return
            self.wfile.write(b'data: caf\xc3');self.wfile.flush();time.sleep(.02)
            self.wfile.write(b'\xa9\n\n');self.wfile.flush()
            for _ in range(500):
                if (root/'gate').exists(): break
                time.sleep(.01)
            else: raise AssertionError('Consumer buffered the entire HTTP stream')
            self.wfile.write(b'data: second\n\n');self.wfile.flush()
        except (BrokenPipeError,ConnectionResetError):
            (root/'disconnected').write_text('cancelled')
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
server.daemon_threads=True
threading.Timer(60,server.shutdown).start()
print(server.server_address[1],flush=True)
server.serve_forever()
