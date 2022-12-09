import socket
import time
import adafruit_rfm69
import busio
from digitalio import DigitalInOut, Direction, Pull
import board
import threading
import queue
import argparse
import ipaddress

TIMEOUT = 0.1
DELAY = 0.25

radio_lock = threading.Lock()
radio_write_queue = queue.Queue()

def str_to_addr(addr_str):
    parts = addr_str.split(':')
    return (str(parts[0]), int(parts[1]))


def radio_thread_fn(radio_handle):
    while True:
        if not radio_write_queue.empty():
            data = radio_write_queue.get()
            print(f'Found data {len(data)} for radio to write, sending')
            radio_lock.acquire()
            radio_handle.send(bytes(data))
            radio_lock.release()
        time.sleep(DELAY)
    

def main_fn():
    parser = argparse.ArgumentParser()
    parser.add_argument('uplink_address')
    parser.add_argument('downlink_address')
    args = parser.parse_args()

    uplink_addr = str_to_addr(args.uplink_address)
    downlink_addr = str_to_addr(args.downlink_address)
    
    # Configure UDP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(uplink_addr)
    sock.settimeout(TIMEOUT)

    # Configure Radio Interface
    CS = DigitalInOut(board.CE1)
    RESET = DigitalInOut(board.D25)
    spi = busio.SPI(board.SCK, MOSI=board.MOSI, MISO=board.MISO)
    radio = adafruit_rfm69.RFM69(spi, CS, RESET, 915.0)

    radio_thread = threading.Thread(target=radio_thread_fn, args=(radio,))
    radio_thread.start()

    print(f'Listening for UDP traffic on {args.uplink_address}')
    print(f'Downlinking radio data to {args.downlink_address}')

    while True:
        try:
            # First check if we have any incoming UDP traffic that needs sending out
            udp_data = sock.recv(1024)
            # If we received any UDP data, then send over radio interface
            if udp_data != None:
                print(f'Got UDP data {len(udp_data)}, queueing up')
                radio_write_queue.put(udp_data)
        except (Exception):
            pass

        # Now we check radio interface for any incoming packets
        radio_lock.acquire()
        radio_data = radio.receive()
        radio_lock.release()
        # If we received a radio packet, then pass along UDP interface
        if radio_data != None:
            print(f'Got radio data {len(radio_data)}, sending along')
            sock.sendto(radio_data, downlink_addr)

        time.sleep(0.01)


if __name__ == "__main__":
    main_fn()
