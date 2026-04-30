use crate::{
    core::{u64x4, BasicCounterUpdate, CounterUpdate, STATE_LANES, STATE_SIZE},
    GenericShiShuAState,
};
use core::convert::TryInto;
use core::convert::Infallible;
use rand::TryRng;
use rand_core::{SeedableRng};

const STATE_WRAPPER_BUFFER_SIZE: usize =
    STATE_LANES * STATE_SIZE * size_of::<u64>();

/// A rand compatible wrapper around the raw ShiShuAState.
///
/// An internal buffer is used to split up big chunks of randomness into the requested size.
pub struct GenericShiShuARng<C: CounterUpdate> {
    state: GenericShiShuAState<C>,
    buffer: [u8; STATE_WRAPPER_BUFFER_SIZE],
    buffer_index: usize,
}

pub type ShiShuARng = GenericShiShuARng<BasicCounterUpdate>;
pub type LongPeriodShiShuARng =
    GenericShiShuARng<crate::core::LongPeriodCounterUpdate>;

impl<C: CounterUpdate + Default> GenericShiShuARng<C> {
    pub fn new(seed: [u64; STATE_LANES]) -> Self {
        GenericShiShuARng {
            state: GenericShiShuAState::new(seed),
            buffer: [0; STATE_WRAPPER_BUFFER_SIZE],
            buffer_index: STATE_WRAPPER_BUFFER_SIZE,
        }
    }

    pub fn new_with_large_seed(seed: [u64; STATE_LANES * 2]) -> Self {
        let main_seed: [u64; STATE_LANES] = seed[0..STATE_LANES].try_into().unwrap();
        let counter_seed: [u64; STATE_LANES] =
            seed[STATE_LANES..].try_into().unwrap();

        // Ensure that counter uses seed, but that similar seeds don't result in similar positions
        // in the same cycle.
        let mut counter_deriver = Self::new(counter_seed);
        counter_deriver.state.counter = u64x4::from(main_seed);
        let mut counter_from_base = [0u64; STATE_LANES];
        counter_deriver
            .try_fill_bytes(bytemuck::cast_slice_mut(&mut counter_from_base)).unwrap();

        let mut new = Self::new(main_seed);
        new.state.counter = u64x4::from(counter_seed);
        new
    }
}

impl<C: CounterUpdate> GenericShiShuARng<C> {
    #[inline(always)]
    pub fn get_byte(&mut self) -> u8 {
        if self.buffer_index >= STATE_WRAPPER_BUFFER_SIZE {
            self.buffer_index = 0;

            let data = self.state.round_unpack();

            let buffer = &mut self.buffer.as_mut();
            for (index, value) in data.iter().enumerate() {
                buffer[(index * size_of::<u64>())
                    ..((index + 1) * size_of::<u64>())]
                    .copy_from_slice(&value.to_le_bytes());
            }
        }

        let index = self.buffer_index;
        self.buffer_index += 1;

        self.buffer[index]
    }
}

impl<C: CounterUpdate> TryRng for GenericShiShuARng<C> {
    type Error = Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        let mut buffer = [0u8; size_of::<u32>()];
        self.try_fill_bytes(&mut buffer)?;
        Ok(u32::from_le_bytes(buffer))
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        let mut buffer = [0u8; size_of::<u64>()];
        self.try_fill_bytes(&mut buffer)?;
        Ok(u64::from_le_bytes(buffer))
    }

    fn try_fill_bytes(&mut self, mut dest: &mut [u8]) -> Result<(), Infallible> {
        while self.buffer_index < STATE_WRAPPER_BUFFER_SIZE && dest.len() > 0 {
            dest[0] = self.buffer[self.buffer_index];
            self.buffer_index += 1;
            dest = &mut dest[1..];
        }

        while dest.len() >= STATE_WRAPPER_BUFFER_SIZE {
            let data = self.state.round_unpack();

            for (index, value) in data.iter().enumerate() {
                dest[(index * size_of::<u64>())
                    ..((index + 1) * size_of::<u64>())]
                    .copy_from_slice(&value.to_le_bytes());
            }

            dest = &mut dest[STATE_WRAPPER_BUFFER_SIZE..];
        }

        for byte in dest.iter_mut() {
            *byte = self.get_byte();
        }

        Ok(())
    }
}

impl<C: CounterUpdate + Default> SeedableRng for GenericShiShuARng<C> {
    type Seed = [u8; STATE_LANES * size_of::<u64>()];

    fn from_seed(seed: Self::Seed) -> Self {
        Self::new(bytemuck::cast(seed))
    }
}
