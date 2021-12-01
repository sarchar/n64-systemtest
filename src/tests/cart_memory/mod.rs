pub mod write;

use alloc::boxed::Box;
use alloc::format;
use crate::tests::{Level, Test};
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use crate::MemoryMap;
use crate::tests::soft_asserts::soft_assert_eq;

// Reading from cart:
// - LW works as expected
// - LH/LB are broken: Every other 16-bit word is not reachable
// - LD: Crashes the console (unless the access is unaligned, in which case there's an AdEL)
// - Addresses have to be uncached. Cached crashes the console

const DATA: [u64; 2] = [0x0123456789ABCDEF, 0x2143658799BADCFE];

pub struct LW {}

impl Test for LW {
    fn name(&self) -> &str { "cart: Read32" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const EXPECTED: [u32; 4] = [0x01234567, 0x89ABCDEF, 0x21436587, 0x99BADCFE];
        let p_cart = MemoryMap::uncached_cart_address(&DATA[0] as *const u64 as *const u32);

        for i in 0..4 {
            let cart_value = unsafe { p_cart.add(i).read_volatile() };
            soft_assert_eq(cart_value, EXPECTED[i], "Reading 32 bit from cart")?;
        }
        Ok(())
    }
}

pub struct LH {}

impl Test for LH {
    fn name(&self) -> &str { "cart: Read16" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // Reading 16 bit value is broken: Every other word can not be reached
        const EXPECTED: [u16; 8] = [0x0123, 0x89AB, 0x89AB, 0x2143, 0x2143, 0x99BA, 0x99BA, 0xDCFE];

        let p_cart = MemoryMap::uncached_cart_address(&DATA[0] as *const u64 as *const u16);
        for i in 0..4 {
            let cart_value = unsafe { p_cart.add(i).read_volatile() };
            soft_assert_eq(cart_value, EXPECTED[i], format!("Reading 16 bit from cart[{}]", i).as_str())?;
        }
        Ok(())
    }
}

pub struct LB {}

impl Test for LB {
    fn name(&self) -> &str { "cart: Read8" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // Reading 8 bit value is broken: Every other 16 bit word can not be reached
        const EXPECTED: [u8; 16] = [0x01, 0x23, 0x89, 0xAB, 0x89, 0xAB, 0x21, 0x43, 0x21, 0x43, 0x99, 0xBA, 0x99, 0xBA, 0xDC, 0xFE];

        let p_cart = MemoryMap::uncached_cart_address(&DATA[0] as *const u64 as *const u8);
        for i in 0..4 {
            let cart_value = unsafe { p_cart.add(i).read_volatile() };
            soft_assert_eq(cart_value, EXPECTED[i], format!("Reading 8 bit from cart[{}]", i).as_str())?;
        }
        Ok(())
    }
}