import socket
import time
import adafruit_rfm69
import busio
from digitalio import DigitalInOut, Direction, Pull
import board

UDP_IP = "127.0.0.1"
UPLINK_PORT = 8080
DOWNLINK_PORT = 8081
TIMEOUT = 0.1
DELAY = 0.5

# Configure UDP socket
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind((UDP_IP, UPLINK_PORT))
sock.settimeout(TIMEOUT)

# Configure Radio Interface
CS = DigitalInOut(board.CE1)
RESET = DigitalInOut(board.D25)
spi = busio.SPI(board.SCK, MOSI=board.MOSI, MISO=board.MISO)
radio = adafruit_rfm69.RFM69(spi, CS, RESET, 915.0)

print(f'Listening for UDP traffic on {UDP_IP}:{UPLINK_PORT}')
print(f'Downlinking radio data to {UDP_IP}:{DOWNLINK_PORT}')

while True:
    try:
        # First check if we have any incoming UDP traffic that needs sending out
        udp_data = sock.recv(1024)
        # If we received any UDP data, then send over radio interface
        if udp_data != None:
            print(f'Got UDP data {udp_data}, sending along')
            radio.send(bytes(udp_data))
    except (Exception):
        pass

    # Now we check radio interface for any incoming packets
    radio_data = radio.receive()
    # If we received a radio packet, then pass along UDP interface
    if radio_data != None:
        print(f'Got radio data {radio_data}, sending along')
        sock.sendto(radio_data, (UDP_IP, DOWNLINK_PORT))

    time.sleep(DELAY)