# Teensy 4.0: Embassy-Beispiel (Blink-LED)

Minimales Beispiel für den [Embassy](https://embassy.dev)-Executor auf dem Teensy 4.0
(i.MX RT1062, 600 MHz). Für diesen Chip gibt es keinen Embassy-HAL; das Beispiel
kombiniert `teensy4-bsp` (Takt, Pins, Laufzeit) mit einem eigenen Embassy-Zeitgeber.

Die Board-LED (Pin 13) blinkt: 0,5 s an, 0,5 s aus.

## Bauen und flashen

```bash
cargo build --release
```

```bash
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/teensy4-embassy-example firmware.hex
```

```bash
teensy_loader_cli --mcu=TEENSY40 -w -v firmware.hex
```

Der Teensy wechselt nicht von selbst in den Bootloader: Vor dem Flashen den
Programmierknopf **kurz** drücken (rote LED leuchtet). Etwa 15 s gedrückt halten löscht
den Flash und stellt das Blink-Programm ab Werk her.

## Aufbau

| Datei                | Inhalt                                                          |
|----------------------|-----------------------------------------------------------------|
| `src/main.rs`        | Start, Caches, Interrupt-Freigabe, Tasks `main_task` und `blink_task` |
| `src/time_driver.rs` | Embassy-Zeitgeber auf GPT1 (1 µs je Tick)                       |
| `.cargo/config.toml` | Ziel `thumbv7em-none-eabihf`, Linker-Skript `t4link.x` aus `teensy4-bsp` |

`memory.x` wird nicht benutzt; das Speicherlayout kommt aus `t4link.x`, das
`teensy4-bsp` (Feature `rt`) beim Bauen erzeugt.

## Embassy auf dem Teensy: worauf es ankommt

Diese Punkte fehlten im ersten Stand; ohne sie blinkt die LED nicht oder unregelmäßig.

1. **Interrupts freigeben.** Der Teensy-Bootloader startet das Programm mit gesperrten
   Interrupts (PRIMASK = 1). Weder `cortex-m-rt` noch der Embassy-Executor geben sie frei
   (RTIC tut es). Ohne `cortex_m::interrupt::enable()` läuft der Zeitgeber-Interrupt nie,
   und `Timer::after(...)` kehrt nicht zurück.
2. **Richtiger Takt für GPT1.** Das BSP stellt PERCLK bereits auf 24-MHz-Quarz / 24 =
   1 MHz (`board::PERCLK_FREQUENCY`). GPT1 zählt mit `ClockSource::PeripheralClock` und
   Teiler 1.
3. **Warteschlange im Zeitgeber.** Embassy-Time 0.4 ruft `schedule_wake` für jeden
   wartenden Task auf. Ein Zeitgeber, der sich nur den letzten Waker merkt, verliert
   Weckzeitpunkte, sobald mehrere Tasks gleichzeitig warten. `time_driver.rs` sammelt
   alle Weckzeiten mit `embassy-time-queue-utils` (Feature `generic-queue-8`, also bis zu
   8 gleichzeitige Timer).
4. **Caches einschalten.** Das BSP und `cortex-m-rt` lassen I- und D-Cache aus. Für Code
   aus dem Flash macht das viel aus; gefahrlos, solange DMA-Puffer im (nicht gecachten)
   DTCM liegen.

### Zeitgeber im Detail

- 64-Bit-Zeit aus dem 32-Bit-Zähler `GPT1.CNT` plus einem Überlaufzähler (Überlauf alle
  ~71 Minuten); ein noch nicht bearbeiteter Überlauf wird beim Lesen berücksichtigt.
- Das Vergleichsregister OCR1 löst beim frühesten Weckzeitpunkt aus. Liegt dieser in
  einer späteren 32-Bit-Epoche, programmiert der Überlauf-Interrupt neu.

## Weiterführend

`~/teensy40/teensy40_embassy_rust` baut darauf auf: BME280 über I2C, Ausgabe über
USB-Serial, ILI9341-Display mit XPT2046-Touch und LVGL-Oberfläche, mehrere Tasks.

## Getestet

14.11.2026 am Teensy 4.0: LED blinkt gleichmäßig.
