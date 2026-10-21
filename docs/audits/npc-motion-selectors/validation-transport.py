"""Private transport adapter: identical JSON lines, immediate Linux ACK of chunks.

The engine writes JSON and newline separately without TCP_NODELAY. The normal
buffered readline waits for the newline, allowing a ~40ms delayed-ACK stall.
This adapter ACKs received bytes before waiting for the rest of the line.
It does not change any input command, game state, emulated frame or RNG value.
"""
import socket


class JsonLineWire:
    def __init__(self, stream):
        self.stream = stream
        self.pending = b''

    def write(self, text):
        self.stream.sendall(text.encode())
        return len(text)

    def flush(self):
        pass

    def readline(self):
        while b'\n' not in self.pending:
            chunk = self.stream.recv(65536)
            if not chunk:
                if self.pending:
                    line, self.pending = self.pending, b''
                    return line.decode()
                return ''
            self.pending += chunk
            if hasattr(socket, 'TCP_QUICKACK'):
                try:
                    self.stream.setsockopt(socket.IPPROTO_TCP, socket.TCP_QUICKACK, 1)
                except OSError:
                    pass
        line, self.pending = self.pending.split(b'\n', 1)
        return line.decode() + '\n'

    def close(self):
        # DebugClient also owns/closes the socket; this wrapper owns no fd.
        self.pending = b''


def accelerated_client(base):
    class Client(base):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            self._install_wire()

        def _install_wire(self):
            self.f.close()
            self.f = JsonLineWire(self.sock)

        def _reconnect(self):
            super()._reconnect()
            self._install_wire()
    return Client
