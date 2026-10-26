
import re
from pathlib import Path

input_path = Path(input("Input dump path: ").strip().strip('"'))
output_path = Path(input("Output .bin path: ").strip().strip('"'))

start_address = 0x8000
end_address = 0x9800
data = bytearray(end_address - start_address)
written = bytearray(len(data))

with input_path.open("r", encoding="utf-8", errors="replace") as file:
    for line in file:
        match = re.match(r"^\s*([0-9A-Fa-f]+):\s*(.*)", line)
        if not match:
            continue

        address = int(match.group(1), 16)
        if not start_address <= address < end_address:
            continue

        offset = address - start_address

        for token in match.group(2).split():
            if not re.fullmatch(r"[0-9A-Fa-f]{2}", token):
                break

            if offset >= len(data):
                break

            data[offset] = int(token, 16)
            written[offset] = 1
            offset += 1

if not all(written):
    raise ValueError(
        f"Dump is missing {written.count(0)} bytes in the requested address range."
    )

output_path.write_bytes(data)
print(f"Converted {len(data)} bytes to {output_path}")
input("Press Enter to exit...")
