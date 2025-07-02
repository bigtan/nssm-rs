#!/usr/bin/env python3
"""
Simple test application for nssm-rs service testing.
This application will run continuously and log messages to demonstrate
service management capabilities.
"""

import time
import sys
import signal
import os
from datetime import datetime

class TestService:
    def __init__(self):
        self.running = True
        self.counter = 0
        
    def signal_handler(self, signum, frame):
        print(f"[{datetime.now()}] Received signal {signum}, shutting down gracefully...")
        self.running = False
        
    def run(self):
        # Set up signal handlers for graceful shutdown
        signal.signal(signal.SIGINT, self.signal_handler)
        signal.signal(signal.SIGTERM, self.signal_handler)
        
        print(f"[{datetime.now()}] Test service started, PID: {os.getpid()}")
        print(f"[{datetime.now()}] Working directory: {os.getcwd()}")
        print(f"[{datetime.now()}] Command line arguments: {sys.argv}")
        
        # Main service loop
        while self.running:
            self.counter += 1
            print(f"[{datetime.now()}] Service heartbeat #{self.counter}")
            
            # Check for command line argument to control behavior
            if len(sys.argv) > 1:
                if sys.argv[1] == "error":
                    if self.counter >= 3:
                        print(f"[{datetime.now()}] Simulating error exit")
                        sys.exit(1)
                elif sys.argv[1] == "crash":
                    if self.counter >= 5:
                        print(f"[{datetime.now()}] Simulating crash")
                        raise Exception("Simulated crash")
                elif sys.argv[1] == "quick":
                    # Quick exit for testing throttling
                    if self.counter >= 2:
                        print(f"[{datetime.now()}] Quick exit")
                        break
            
            try:
                time.sleep(2)  # Wait 2 seconds between heartbeats
            except KeyboardInterrupt:
                print(f"[{datetime.now()}] Keyboard interrupt received")
                break
                
        print(f"[{datetime.now()}] Service shutting down after {self.counter} heartbeats")
        print(f"[{datetime.now()}] Exit code: 0")

if __name__ == "__main__":
    service = TestService()
    service.run()
