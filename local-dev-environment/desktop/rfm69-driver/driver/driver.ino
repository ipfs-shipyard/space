// This is borrowed heavily from the rf69 rx and tx demo code
// -*- mode: C++ -*-

#include <SPI.h>
#include <RH_RF69.h>

/************ Radio Setup ***************/

// Change to 434.0 or other frequency, must match RX's freq!
#define RF69_FREQ 915.0

// Feather 32u4 w/Radio pin defs
#define RFM69_CS      8
#define RFM69_INT     7
#define RFM69_RST     4
#define LED           13

// Singleton instance of the radio driver
RH_RF69 rf69(RFM69_CS, RFM69_INT);

void setup() 
{
  Serial.begin(115200);

  pinMode(LED, OUTPUT);     
  pinMode(RFM69_RST, OUTPUT);
  digitalWrite(RFM69_RST, LOW);

  // manual reset
  digitalWrite(RFM69_RST, HIGH);
  delay(10);
  digitalWrite(RFM69_RST, LOW);
  delay(10);
  
  if (!rf69.init()) {
    Serial.println("RFM69 radio init failed");
    while (1);
  }
  // Defaults after init are 434.0MHz, modulation GFSK_Rb250Fd250, +13dbM (for low power module)
  // No encryption
  if (!rf69.setFrequency(RF69_FREQ)) {
    Serial.println("setFrequency failed");
  }

  // If you are using a high power RF69 eg RFM69HW, you *must* set a Tx power with the
  // ishighpowermodule flag set like this:
  rf69.setTxPower(20, true);  // range from 14-20 for power, 2nd arg must be true for 69HCW

  pinMode(LED, OUTPUT);
}


void loop() {  
  delay(10);  // Wait 10ms between cycles

  uint8_t buf[RH_RF69_MAX_MESSAGE_LEN];
  uint8_t len = sizeof(buf);

  int availableBytes = Serial.available();
  if (availableBytes > 0) {
    int len = Serial.readBytes(buf, availableBytes);
    buf[len] = 0;
    rf69.send((uint8_t*)buf, len);
    rf69.waitPacketSent();
    Blink(LED, 30, 2);
  }

  if (rf69.waitAvailableTimeout(10))  { 
    // Should be a reply message for us now 
    if (rf69.recv(buf, &len)) {
      buf[len] = 0;
      Serial.write(buf, len);
      Blink(LED, 70, 2);
    }
  }
}

void Blink(byte PIN, byte DELAY_MS, byte loops) {
  for (byte i=0; i<loops; i++)  {
    digitalWrite(PIN,HIGH);
    delay(DELAY_MS);
    digitalWrite(PIN,LOW);
    delay(DELAY_MS);
  }
}