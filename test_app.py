import time
import sys

print("Test application started")
print(f"Arguments: {sys.argv}")

count = 0
try:
    while True:
        count += 1
        print(f"Test app running - iteration {count}")
        time.sleep(5)
except KeyboardInterrupt:
    print("Test application received Ctrl+C, exiting...")
    sys.exit(0)
except Exception as e:
    print(f"Test application error: {e}")
    sys.exit(1)
