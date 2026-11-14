MEMORY
{
  /* Teensy 4.0 has 1MB RAM and 2MB Flash */
  RAM    (rwx) : ORIGIN = 0x20000000, LENGTH = 1M
  FLASH  (rx)  : ORIGIN = 0x60000000, LENGTH = 2M
}
