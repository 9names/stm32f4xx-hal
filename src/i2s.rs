//! I2S (inter-IC Sound) communication using SPI peripherals
//!
//! This module is only available if the `i2s` feature is enabled.

use crate::gpio::{Const, NoPin, PinA, PushPull, SetAlternate};
#[cfg(feature = "stm32_i2s_v12x")]
use stm32_i2s_v12x::{Instance, RegisterBlock};

use crate::pac::RCC;
use crate::rcc;
use crate::time::Hertz;
use crate::{rcc::Clocks, spi};

// I2S pins are mostly the same as the corresponding SPI pins:
// MOSI -> SD
// NSS -> WS (the current SPI code doesn't define NSS pins)
// SCK -> CK
// The master clock output is separate.

/// A pin that can be used as SD (serial data)
///
/// Each MOSI pin can also be used as SD
pub type Sd = spi::Mosi;

/// A pin that can be used as WS (word select, left/right clock)
pub type Ws = spi::Nss;

/// A pin that can be used as CK (bit clock)
///
/// Each SCK pin can also be used as CK
pub type Ck = spi::Sck;

/// A pin that can be used as MCK (master clock output)
pub struct Mck;
impl crate::Sealed for Mck {}

/// A placeholder for when the MCLK pin is not needed
pub type NoMasterClock = NoPin;

/// A set of pins configured for I2S communication: (WS, CK, MCLK, SD)
///
/// NoMasterClock can be used instead of the master clock pin.
pub trait Pins<SPI> {}

impl<SPI, PWS, PCK, PMCLK, PSD> Pins<SPI> for (PWS, PCK, PMCLK, PSD)
where
    PWS: PinA<Ws, SPI>,
    PCK: PinA<Ck, SPI>,
    PMCLK: PinA<Mck, SPI>,
    PSD: PinA<Sd, SPI>,
{
}

pub trait I2sFreq {
    fn i2s_freq(clocks: &Clocks) -> Hertz;
}

/// Implements Instance for I2s<$SPIX, _> and creates an I2s::$spix function to create and enable
/// the peripheral
///
/// $SPIX: The fully-capitalized name of the SPI peripheral (example: SPI1)
/// $i2sx: The lowercase I2S name of the peripheral (example: i2s1). This is the name of the
/// function that creates an I2s and enables the peripheral clock.
/// $clock: The name of the Clocks function that returns the frequency of the I2S clock input
/// to this SPI peripheral (i2s_cl, i2s_apb1_clk, or i2s2_apb_clk)
macro_rules! i2s {
    ($SPIX:ty, $clock:ident) => {
        impl I2sFreq for $SPIX {
            fn i2s_freq(clocks: &Clocks) -> Hertz {
                clocks
                    .$clock()
                    .expect("I2S clock input for SPI not enabled")
            }
        }

        #[cfg(feature = "stm32_i2s_v12x")]
        unsafe impl<PINS> Instance for I2s<$SPIX, PINS> {
            const REGISTERS: *mut RegisterBlock = <$SPIX>::ptr() as *mut _;
        }
    };
}

impl<SPI, WS, CK, MCLK, SD, const WSA: u8, const CKA: u8, const MCLKA: u8, const SDA: u8>
    I2s<SPI, (WS, CK, MCLK, SD)>
where
    SPI: I2sFreq + rcc::Enable + rcc::Reset,
    WS: PinA<Ws, SPI, A = Const<WSA>> + SetAlternate<PushPull, WSA>,
    CK: PinA<Ck, SPI, A = Const<CKA>> + SetAlternate<PushPull, CKA>,
    MCLK: PinA<Mck, SPI, A = Const<MCLKA>> + SetAlternate<PushPull, MCLKA>,
    SD: PinA<Sd, SPI, A = Const<SDA>> + SetAlternate<PushPull, SDA>,
{
    /// Creates an I2s object around an SPI peripheral and pins
    ///
    /// This function enables and resets the SPI peripheral, but does not configure it.
    ///
    /// The returned I2s object implements [stm32_i2s_v12x::Instance], so it can be used
    /// to configure the peripheral and communicate.
    ///
    /// # Panics
    ///
    /// This function panics if the I2S clock input (from the I2S PLL or similar)
    /// is not configured.
    pub fn new(spi: SPI, mut pins: (WS, CK, MCLK, SD), clocks: &Clocks) -> Self {
        let input_clock = SPI::i2s_freq(clocks);
        unsafe {
            // NOTE(unsafe) this reference will only be used for atomic writes with no side effects.
            let rcc = &(*RCC::ptr());
            // Enable clock, enable reset, clear, reset
            SPI::enable(rcc);
            SPI::reset(rcc);
        }

        pins.0.set_alt_mode();
        pins.1.set_alt_mode();
        pins.2.set_alt_mode();
        pins.3.set_alt_mode();

        I2s {
            _spi: spi,
            _pins: pins,
            input_clock,
        }
    }
}

// Actually define the SPI instances that can be used for I2S
// Each one has to be split into two declarations because the F412, F413, F423, and F446
// have two different I2S clocks while other models have only one.

#[cfg(any(feature = "stm32f410", feature = "stm32f411"))]
i2s!(crate::pac::SPI1, i2s_clk);
#[cfg(any(
    feature = "stm32f412",
    feature = "stm32f413",
    feature = "stm32f423",
    feature = "stm32f446",
))]
i2s!(crate::pac::SPI1, i2s_apb2_clk);

// All STM32F4 models support SPI2/I2S2
#[cfg(not(any(
    feature = "stm32f412",
    feature = "stm32f413",
    feature = "stm32f423",
    feature = "stm32f446",
)))]
i2s!(crate::pac::SPI2, i2s_clk);
#[cfg(any(
    feature = "stm32f412",
    feature = "stm32f413",
    feature = "stm32f423",
    feature = "stm32f446",
))]
i2s!(crate::pac::SPI2, i2s_apb1_clk);

// All STM32F4 models except STM32F410 support SPI3/I2S3
#[cfg(any(
    feature = "stm32f401",
    feature = "stm32f405",
    feature = "stm32f407",
    feature = "stm32f411",
    feature = "stm32f415",
    feature = "stm32f417",
    feature = "stm32f427",
    feature = "stm32f429",
    feature = "stm32f437",
    feature = "stm32f439",
    feature = "stm32f469",
    feature = "stm32f479",
))]
i2s!(crate::pac::SPI3, i2s_clk);
#[cfg(any(
    feature = "stm32f412",
    feature = "stm32f413",
    feature = "stm32f423",
    feature = "stm32f446",
))]
i2s!(crate::pac::SPI3, i2s_apb1_clk);

#[cfg(feature = "stm32f411")]
i2s!(crate::pac::SPI4, i2s_clk);
#[cfg(any(feature = "stm32f412", feature = "stm32f413", feature = "stm32f423"))]
i2s!(crate::pac::SPI4, i2s_apb2_clk);

#[cfg(any(feature = "stm32f410", feature = "stm32f411"))]
i2s!(crate::pac::SPI5, i2s_clk);
#[cfg(any(feature = "stm32f412", feature = "stm32f413", feature = "stm32f423"))]
i2s!(crate::pac::SPI5, i2s_apb2_clk);

/// An I2s wrapper around an SPI object and pins
pub struct I2s<I, PINS> {
    _spi: I,
    _pins: PINS,
    /// Frequency of clock input to this peripheral from the I2S PLL or related source
    input_clock: Hertz,
}

impl<I, PINS> I2s<I, PINS> {
    /// Returns the frequency of the clock signal that the SPI peripheral is receiving from the
    /// I2S PLL or similar source
    pub fn input_clock(&self) -> Hertz {
        self.input_clock
    }
}

// DMA support: reuse existing mappings for SPI
#[cfg(feature = "stm32_i2s_v12x")]
mod dma {
    use super::*;
    use crate::dma::traits::{DMASet, PeriAddress};
    use core::ops::Deref;

    /// I2S DMA reads from and writes to the data register
    unsafe impl<SPI, PINS, MODE> PeriAddress for stm32_i2s_v12x::I2s<I2s<SPI, PINS>, MODE>
    where
        I2s<SPI, PINS>: Instance,
        PINS: Pins<SPI>,
        SPI: Deref<Target = crate::pac::spi1::RegisterBlock>,
    {
        /// SPI_DR is only 16 bits. Multiple transfers are needed for a 24-bit or 32-bit sample,
        /// as explained in the reference manual.
        type MemSize = u16;

        fn address(&self) -> u32 {
            let registers = &*self.instance()._spi;
            &registers.dr as *const _ as u32
        }
    }

    /// DMA is available for I2S based on the underlying implementations for SPI
    unsafe impl<SPI, PINS, MODE, STREAM, DIR, const CHANNEL: u8> DMASet<STREAM, DIR, CHANNEL>
        for stm32_i2s_v12x::I2s<I2s<SPI, PINS>, MODE>
    where
        SPI: DMASet<STREAM, DIR, CHANNEL>,
    {
    }
}
