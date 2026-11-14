//! Embassy-Zeitgeber auf GPT1 des i.MX RT1062: PERCLK = 1 MHz (1 µs je Tick).
//!
//! - 64-Bit-Zeit aus dem 32-Bit-Zähler plus Überlaufzähler (Überlauf alle ~71 min).
//! - Weckzeiten aller Tasks in einer Warteschlange (`embassy-time-queue-utils`);
//!   Vergleichsregister OCR1 löst beim frühesten Zeitpunkt aus. Liegt dieser in einer
//!   späteren 32-Bit-Epoche, programmiert der Überlauf-Interrupt neu.

use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, Ordering};
use core::task::Waker;

use critical_section::{CriticalSection, Mutex};
use embassy_time_driver::{Driver, time_driver_impl};
use embassy_time_queue_utils::Queue;
use teensy4_bsp as bsp;

use bsp::hal::gpt::{ClockSource, Gpt1, Mode};
use bsp::interrupt;
use bsp::ral;

const SR_OF1: u32 = 1 << 0; // Output Compare 1
const SR_ROV: u32 = 1 << 5; // Rollover
const IR_OF1IE: u32 = 1 << 0;

struct GptDriver {
    overflows: AtomicU32,
    queue: Mutex<RefCell<Queue>>,
}

time_driver_impl!(static DRIVER: GptDriver = GptDriver {
    overflows: AtomicU32::new(0),
    queue: Mutex::new(RefCell::new(Queue::new())),
});

fn gpt() -> &'static ral::gpt::RegisterBlock {
    // Safety: GPT1 gehört nach `init` exklusiv diesem Treiber.
    unsafe { &*ral::gpt::GPT1 }
}

/// GPT1 einrichten und starten. Vor dem ersten `Timer` aufrufen.
pub fn init(gpt1: &mut Gpt1) {
    // Das BSP stellt PERCLK auf 24-MHz-Quarz / 24 = 1 MHz (board::PERCLK_FREQUENCY).
    // GPT zählt mit PERCLK = "PeripheralClock", ohne weiteren Teiler.
    const _: () = assert!(bsp::board::PERCLK_FREQUENCY == 1_000_000);
    gpt1.disable();
    gpt1.set_clock_source(ClockSource::PeripheralClock);
    gpt1.set_divider(1);
    gpt1.set_mode(Mode::FreeRunning);
    gpt1.set_rollover_interrupt_enable(true);
    gpt1.enable();
    unsafe { cortex_m::peripheral::NVIC::unmask(interrupt::GPT1) };
}

impl GptDriver {
    fn ticks(&self) -> u64 {
        loop {
            let hi = self.overflows.load(Ordering::Acquire);
            let lo = gpt().CNT.read();
            let pending = gpt().SR.read() & SR_ROV != 0;
            if hi != self.overflows.load(Ordering::Acquire) {
                continue; // Interrupt dazwischen: nochmals
            }
            // Überlauf passiert, aber Interrupt noch nicht gelaufen (z. B. in einer
            // Critical Section): obere Hälfte selbst erhöhen
            let hi = if pending && lo < 0x8000_0000 { hi + 1 } else { hi };
            return ((hi as u64) << 32) | lo as u64;
        }
    }

    /// Vergleich für `at` programmieren. `false`, wenn `at` schon vorbei ist.
    fn set_alarm(&self, at: u64) -> bool {
        let now = self.ticks();
        if at <= now {
            return false;
        }
        let g = gpt();
        if at >> 32 == now >> 32 {
            g.OCR[0].write(at as u32);
            g.SR.write(SR_OF1);
            g.IR.write(g.IR.read() | IR_OF1IE);
            // Zwischen Lesen und Programmieren abgelaufen?
            if self.ticks() >= at {
                return false;
            }
        } else {
            // Spätere Epoche (oder keine Weckzeit): der Überlauf-Interrupt prüft erneut
            g.IR.write(g.IR.read() & !IR_OF1IE);
        }
        true
    }

    fn dispatch(&self, cs: CriticalSection) {
        let mut queue = self.queue.borrow(cs).borrow_mut();
        let mut next = queue.next_expiration(self.ticks());
        while !self.set_alarm(next) {
            next = queue.next_expiration(self.ticks());
        }
    }
}

impl Driver for GptDriver {
    fn now(&self) -> u64 {
        self.ticks()
    }

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        critical_section::with(|cs| {
            let scheduled = self.queue.borrow(cs).borrow_mut().schedule_wake(at, waker);
            if scheduled {
                self.dispatch(cs);
            }
        });
    }
}

#[bsp::rt::interrupt]
fn GPT1() {
    let g = gpt();
    let sr = g.SR.read();
    if sr & SR_ROV != 0 {
        g.SR.write(SR_ROV);
        DRIVER.overflows.fetch_add(1, Ordering::Release);
    }
    if sr & SR_OF1 != 0 {
        g.SR.write(SR_OF1);
    }
    critical_section::with(|cs| DRIVER.dispatch(cs));
}
