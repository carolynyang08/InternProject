#![no_std]
#![no_main]

#[allow(unused_extern_crates)] // NOTE(allow) bug rust-lang/rust#53964
extern crate panic_itm; // panic handler

use core::fmt::Debug;
pub use cortex_m::{
    asm, interrupt::Mutex, iprint, iprintln, peripheral::ITM, peripheral::NVIC, Peripherals,
};
pub use cortex_m_rt::entry;

//use fugit::{Duration, ExtU32};
use embedded_time::{duration::*, timer}; //, rate::*};

use core::{cell::RefCell, ops::DerefMut};

use stm32f3xx_hal::{
    delay::Delay,
    gpio::{self, gpiob, Alternate, Edge, Input, OpenDrain, Output, PushPull, AF4},
    i2c::{self, I2c},
    // stm32,
    pac::{self, interrupt, I2C1, TIM2, TIM3, TIM4},
    //interrupt,
    prelude::*,
    pwm::{tim15, tim3, PwmChannel, Tim15Ch1, WithPins},
    timer::{Event, Timer},
};
use vl53l1x_uld::{self, IOVoltage, RangeStatus, VL53L1X};

static DELAY: Mutex<RefCell<Option<Delay>>> = Mutex::new(RefCell::new(None));

static TIMER_TIM1: Mutex<RefCell<Option<Timer<TIM2>>>> = Mutex::new(RefCell::new(None));
static TIMER_TIM2: Mutex<RefCell<Option<Timer<TIM3>>>> = Mutex::new(RefCell::new(None));
static TIMER_TIM3: Mutex<RefCell<Option<Timer<TIM4>>>> = Mutex::new(RefCell::new(None));

type SpeakerSignal = PwmChannel<Tim15Ch1, WithPins>;
static SIGNAL: Mutex<RefCell<Option<SpeakerSignal>>> = Mutex::new(RefCell::new(None));

type I2CPins = I2c<
    I2C1,
    (
        gpio::PB6<Alternate<OpenDrain, 4>>,
        gpio::PB7<Alternate<OpenDrain, 4>>,
    ),
>;

type VL53I2CPins = VL53L1X<I2CPins>;

static VL_I2C: Mutex<RefCell<Option<VL53I2CPins>>> = Mutex::new(RefCell::new(None));

type ITMINIT = cortex_m::peripheral::ITM;
static CP_ITM: Mutex<RefCell<Option<ITMINIT>>> = Mutex::new(RefCell::new(None));

static mut DISTANCE: u16 = 0;

static mut DELAY_TIME: Milliseconds = embedded_time::duration::Milliseconds(0);
static mut BEEP_TIME: u16 = 0;


#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let itm = cp.ITM;

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let delay = Delay::new(cp.SYST, clocks);

    let mut syscfg = dp.SYSCFG.constrain(&mut rcc.apb2);

    // Configure leds
    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut gpioe = dp.GPIOE.split(&mut rcc.ahb);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);

    //Configure I2C
    let mut scl =
        gpiob
            .pb6
            .into_af_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    let mut sda =
        gpiob
            .pb7
            .into_af_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);

    scl.internal_pull_up(&mut gpiob.pupdr, true);
    sda.internal_pull_up(&mut gpiob.pupdr, true);
  

    let i2c = i2c::I2c::new(
        dp.I2C1,
        (scl, sda),
        100.kHz().try_into().unwrap(),
        clocks,
        &mut rcc.apb1,
    );

    //Initialize VL53L1X TOF sensor
    const ERR: &str = "Failed to communicate";
    let vl = VL53L1X::new(i2c, vl53l1x_uld::DEFAULT_ADDRESS);

    //Initialize speaker
    let speaker =
        gpiob
            .pb14
            .into_af_push_pull(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrh);

    //Set upt PWM Channel
    let tim15_channel = tim15(dp.TIM15, 9000, 440.Hz(), &clocks);
    let mut signal = tim15_channel.0.output_to_pb14(speaker);

    // Move the ownership of the local items to global variables
    cortex_m::interrupt::free(|cs| {
        *CP_ITM.borrow(cs).borrow_mut() = Some(itm);
        *VL_I2C.borrow(cs).borrow_mut() = Some(vl);
        *SIGNAL.borrow(cs).borrow_mut() = Some(signal);
    });

    // Move the ownership of the delay to global DELAY
    cortex_m::interrupt::free(|cs| *DELAY.borrow(cs).borrow_mut() = Some(delay));

    //ENABLE TIMER INTERRUPTS
    // First timer interrupt
    let mut timer1 = Timer::new(dp.TIM2, clocks, &mut rcc.apb1);
    timer1.start(1000.milliseconds());
    timer1.enable_interrupt(Event::Update);
    let timer1_interrupt = timer1.interrupt();

    // Second timer interrupt
    let mut timer2 = Timer::new(dp.TIM3, clocks, &mut rcc.apb1);
    timer2.start(500.milliseconds()); // NOTE: 1,000 is the max value accepted by milliseconds() - anything higher will be truncated
    timer2.enable_interrupt(Event::Update);
    let timer2_interrupt = timer2.interrupt();

    //Third timer interrupt
    let mut timer3 = Timer::new(dp.TIM4, clocks, &mut rcc.apb1);
    timer3.start(900.milliseconds());
    timer3.enable_interrupt(Event::Update);
    let timer3_interrupt = timer3.interrupt();

    // Moving ownership to the global BUTTON so we can clear the interrupt pending bit.
    cortex_m::interrupt::free(|cs| {
        *TIMER_TIM1.borrow(cs).borrow_mut() = Some(timer1);
        *TIMER_TIM2.borrow(cs).borrow_mut() = Some(timer2);
        *TIMER_TIM3.borrow(cs).borrow_mut() = Some(timer3)
    });

    NVIC::unpend(timer1_interrupt);
    NVIC::unpend(timer2_interrupt);
    NVIC::unpend(timer3_interrupt);

    unsafe {
        NVIC::unmask(timer1_interrupt);
        NVIC::unmask(timer2_interrupt);
        NVIC::unmask(timer3_interrupt);
    };

    loop {
        asm::wfi();
    }
}

//Speaker calculation task + LCD Display
#[interrupt]
fn TIM2() {
    cortex_m::interrupt::free(|cs| {
        let frequency = 262_u32;
        if let Some(ref mut tim2) = TIMER_TIM1.borrow(cs).borrow_mut().deref_mut() {
            tim2.clear_event(Event::Update);
        }
        if let Some(ref mut itm) = CP_ITM.borrow(cs).borrow_mut().deref_mut() {
            unsafe {
                iprintln!(&mut itm.stim[0], "DISTANCE 2: {}cm", DISTANCE);
            }
        }

        let mut delay_amt = 0_u16;
        let mut beep_time = 0_u16;

        if let Some(ref mut delay) = DELAY.borrow(cs).borrow_mut().deref_mut() {
            if let Some(ref mut itm) = CP_ITM.borrow(cs).borrow_mut().deref_mut() {
                iprintln!(&mut itm.stim[0], "In speaker task interrupt");
                unsafe {
                    iprintln!(&mut itm.stim[0], "DISTANCE 3: {}cm", DISTANCE);
                }

                unsafe {
                    iprintln!(&mut itm.stim[0], "In unsafe");

                    if DISTANCE < 4 {
                        DELAY_TIME = 150.milliseconds();
                        BEEP_TIME = 60; //beep_time = 100;
                    } else if DISTANCE < 7 {
                        DELAY_TIME = 250.milliseconds();
                        BEEP_TIME = 100;
                    } else if DISTANCE < 12 {
                        DELAY_TIME = 300.milliseconds();
                        BEEP_TIME = 120;
                    } else if DISTANCE < 16 {
                        DELAY_TIME = 400.milliseconds();
                        BEEP_TIME = 160;
                    } else if DISTANCE < 20 {
                        DELAY_TIME = 500.milliseconds();
                        BEEP_TIME = 200;
                    } else if DISTANCE < 25 {
                        DELAY_TIME = 600.milliseconds();
                        BEEP_TIME = 240;
                    }
                }
                if let Some(ref mut vl) = VL_I2C.borrow(cs).borrow_mut().deref_mut() {
                    
                    let mut ones: u16 = 0;
                    let mut tens: u16 = 0;
        
                    const ERR: &str = "Failed to communicate";

                    unsafe {
                        ones = (DISTANCE % 10) + 48;
                        let converted_ones = ones as u8;

                        tens = (DISTANCE / 10) + 48;
                        let converted_tens = tens as u8;
           
                        vl.set_address(0x72).expect(ERR);
                        iprintln!(&mut itm.stim[0], "In LCD I2C");
                       //vl.set_address(0x72).expect(ERR);
                        
                        vl.write_bytes([0x7C, 0x2D], &[converted_tens, converted_ones, 0x63, 0x6D]);
                    }
                
                    vl.set_address(0x29).expect(ERR);
                }
            }
        }
    })
}

//Speaker output task
#[interrupt]
fn TIM3() {
    cortex_m::interrupt::free(|cs| {
        if let Some(ref mut itm) = CP_ITM.borrow(cs).borrow_mut().deref_mut() {
            unsafe {
                iprintln!(&mut itm.stim[0], "In TIM3 INTERRUPT");
            }

            let LCD_ADDR = 0x72_u8;
            if let Some(ref mut tim3) = TIMER_TIM2.borrow(cs).borrow_mut().deref_mut() {
                unsafe {
                    tim3.start(DELAY_TIME);
                    iprintln!(&mut itm.stim[0], "DELAY TIME: {}", DELAY_TIME);
                }
                tim3.clear_event(Event::Update);
            }
            if let Some(ref mut signal) = SIGNAL.borrow(cs).borrow_mut().deref_mut() {
                if let Some(ref mut delay) = DELAY.borrow(cs).borrow_mut().deref_mut() {
                    unsafe {
                        if DISTANCE < 25 {
                            signal.enable();
                            signal.set_duty(signal.get_max_duty() / 2);
                            unsafe {
                                delay.delay_ms(BEEP_TIME);
                                iprintln!(&mut itm.stim[0], "BEEP TIME: {}", BEEP_TIME);
                            }
                            signal.disable();
                        }
                    }
                }
            }
        }
    })
}

//Distance Measuring Task
#[interrupt]
fn TIM4() {
    cortex_m::interrupt::free(|cs| {
        let LCD_ADDDRESS = 0x72_u8;
        let VL53L1X_ADDRESS = 0x29_u8;
        if let Some(ref mut tim4) = TIMER_TIM3.borrow(cs).borrow_mut().deref_mut() {
            tim4.clear_event(Event::Update);
        }
        //itm output set-up
        if let Some(ref mut itm) = CP_ITM.borrow(cs).borrow_mut().deref_mut() {
            iprintln!(&mut itm.stim[0], "itm is working");
            if let Some(ref mut vl) = VL_I2C.borrow(cs).borrow_mut().deref_mut() {
                let mut ones: u16 = 0;
                let mut tens: u16 = 0;

                //Initilaize user vl53l1x sensor
                const ERR: &str = "Failed to communicate";
                vl.init(IOVoltage::Volt2_8).expect(ERR);
                vl.start_ranging().expect(ERR);
                let mut distance: u16 = 0;
                //Wait until distance data is ready to be read
                while !vl.is_data_ready().expect(ERR) {}
                // Check if distance measurement is valid.
                if vl.get_range_status().expect(ERR) == RangeStatus::Valid {
                    // Retrieve measured distance.
                    distance = vl.get_distance().unwrap() / 10;
                    unsafe {
                        DISTANCE = distance;
                        iprintln!(&mut itm.stim[0], "The distace is: {}cm", DISTANCE);
                    }
                }
            }
        }
    })
}
