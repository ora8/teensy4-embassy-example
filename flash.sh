arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/teensy4-embassy-example firmware.hex

teensy_loader_cli --mcu=TEENSY40 -w -v firmware.hex