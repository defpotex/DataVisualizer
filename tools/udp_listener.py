"""
Simple UDP listener for testing udp_streamer output.
Runs natively on Windows — no WSL loopback issues.

Usage:
    python tools/udp_listener.py [port]    (default: 5005)
"""
import socket
import sys

port = int(sys.argv[1]) if len(sys.argv) > 1 else 5005

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("0.0.0.0", port))

print(f"Listening on UDP port {port} — Ctrl+C to stop\n")

count = 0
try:
    while True:
        data, addr = sock.recvfrom(65535)
        line = data.decode("utf-8", errors="replace").rstrip("\n")
        count += 1
        print(f"[{count:>6}] {line}")
except KeyboardInterrupt:
    print(f"\nDone. {count} packets received.")
finally:
    sock.close()
