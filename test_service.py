#!/usr/bin/env python3
"""
Simple test application for nssm-rs
This script will run continuously and log messages
"""

import time
import sys
import signal

def signal_handler(signum, frame):
    print(f"Received signal {signum}, exiting gracefully...")
    sys.exit(0)

# Handle Ctrl+C gracefully
signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)

def main():
    print("Test application started")
    print(f"Arguments: {sys.argv[1:]}")
    print(f"Working directory: {os.getcwd()}")
    
    counter = 0
    while True:
        counter += 1
        print(f"Running... counter={counter}")
        time.sleep(5)

if __name__ == "__main__":
    import os
    main()
