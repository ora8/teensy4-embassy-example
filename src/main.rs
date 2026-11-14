#![no_std]
#![no_main]

mod time_driver;

use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use teensy4_bsp as bsp;
use teensy4_panic as _;

use bsp::board;

// ---------------------------------------------------------------------------
// Embassy Application Tasks
// ---------------------------------------------------------------------------

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::task]
async fn blink_task(led: board::Led) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn main_task(spawner: Spawner, led: board::Led) {
    spawner.spawn(blink_task(led)).unwrap();

    loop {
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[bsp::rt::entry]
fn main() -> ! {
    let board::Resources {
        pins,
        mut gpio2,
        mut gpt1,
        ..
    } = board::t40(board::instances());

    // Caches des Cortex-M7 einschalten (macht weder das BSP noch cortex-m-rt)
    let mut core = cortex_m::Peripherals::take().unwrap();
    core.SCB.enable_icache();
    core.SCB.enable_dcache(&mut core.CPUID);

    // Embassy-Zeitgeber auf GPT1 (1 MHz) starten
    time_driver::init(&mut gpt1);

    // Der Teensy-Bootloader startet das Programm mit gesperrten Interrupts (PRIMASK = 1),
    // und weder cortex-m-rt noch der Embassy-Executor geben sie frei. Ohne diese Zeile
    // läuft der Zeitgeber-Interrupt nie, und Timer::after(...) kehrt nicht zurück.
    unsafe { cortex_m::interrupt::enable() };

    let led = board::led(&mut gpio2, pins.p13);

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(main_task(spawner, led)).unwrap();
    });
}
