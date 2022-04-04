use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;

use crate::{MemoryMap, VIDEO};
use crate::graphics::color::Color;
use crate::graphics::color::RGBA1555;
use crate::graphics::cursor::Cursor;
use crate::graphics::font::Font;
use crate::graphics::system_font::FONT_GENEVA_9;
use crate::rsp::rsp::RSP;
use crate::rsp::rsp_assembler::{CP0Register, E, Element, GPR, RSPAssembler, VR};
use crate::rsp::spmem::SPMEM;
use crate::tests::{Level, Test};
use crate::tests::soft_asserts::soft_assert_eq2;

pub struct VMULF {}

impl Test for VMULF {
    fn name(&self) -> &str { "RSP VMULF (Stress test)" }

    fn level(&self) -> Level { Level::StressTest }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // TODO: After a reboot the first iteration fails with accum.lo being zero. What's happening there?
        // Ways to speed this up:
        // - The CPU is calculating into memory. It would be faster if we didn't have to store the result
        //   but calculated on the fly and compare to the RSP result. We could do that by having the rsp
        //   spit out a back- and a frontresult and compare to that.
        const STEPS_PER_RSP: usize = 32;
        let mut rsp_out_data: [[[u16; 8]; 4]; STEPS_PER_RSP] = Default::default();
        let mut cpu_out_data: [[[u16; 8]; 4]; STEPS_PER_RSP] = Default::default();

        SPMEM::write_vector_16(0x20, &[1, 1, 1, 1, 1, 1, 1, 1]);

        // Assemble RSP program
        let mut assembler = RSPAssembler::new(0);

        assembler.write_lqv(VR::V0, E::_0, 0x000, GPR::R0);
        assembler.write_lqv(VR::V1, E::_0, 0x010, GPR::R0);
        assembler.write_lqv(VR::V2, E::_0, 0x020, GPR::R0);
        assembler.write_ori(GPR::A0, GPR::R0, 0x40);


        for i in 0..STEPS_PER_RSP {
            if i != 0 {
                // Increment V1 by 1
                assembler.write_vadd(VR::V1, VR::V1, VR::V2, Element::All);
                // Increment output address
                assembler.write_addiu(GPR::A0, GPR::A0, 0x40);
            }

            assembler.write_vmulf(VR::V3, VR::V1, VR::V0, Element::All);
            assembler.write_vsar(VR::V4, VR::V0, VR::V0, E::_8);
            assembler.write_vsar(VR::V5, VR::V0, VR::V0, E::_9);
            assembler.write_vsar(VR::V6, VR::V0, VR::V0, E::_10);

            assembler.write_sqv(VR::V3, E::_0, 0x00, GPR::A0);
            assembler.write_sqv(VR::V4, E::_0, 0x10, GPR::A0);
            assembler.write_sqv(VR::V5, E::_0, 0x20, GPR::A0);
            assembler.write_sqv(VR::V6, E::_0, 0x30, GPR::A0);
        }

        // Initiate DMA back to CPU
        assembler.write_ori(GPR::A1, GPR::R0, 0x40);
        assembler.write_mtc0(CP0Register::SPAddress, GPR::A1);
        assembler.write_li(GPR::A1, (&mut rsp_out_data).as_mut_ptr() as u32);
        assembler.write_mtc0(CP0Register::DRAMAddress, GPR::A1);
        assembler.write_li(GPR::A1, (STEPS_PER_RSP * 4 * 16) as u32 - 1);
        assembler.write_mtc0(CP0Register::WriteLength, GPR::A1);

        assembler.write_break();

        let font = Font::from_data(&FONT_GENEVA_9).unwrap();
        let mut cursor = Cursor::new_with_font(&font, RGBA1555::BLACK);
        for a_base in (0..=0xFFFF).step_by(8) {
            {
                let v = VIDEO.lock();
                {
                    let mut lock = v.framebuffers().backbuffer().lock();
                    let buffer = lock.as_mut().unwrap();
                    buffer.clear_with_color(RGBA1555::WHITE);

                    cursor.x = 16;
                    cursor.y = 16;
                    cursor.draw_text(buffer, format!("{:3.2}%", a_base as f32 / 65536.0f32 * 100.0f32).as_str());
                }
                v.swap_buffers();
            }

            let input1 = [a_base, a_base + 1, a_base + 2, a_base + 3, a_base + 4, a_base + 5, a_base + 6, a_base + 7];
            SPMEM::write_vector_16(0x00, &input1);
            for b_base in (0..=0xFFFF).step_by(STEPS_PER_RSP) {
                // Send over data to RSP and tell it to get going
                let input2 = [b_base, b_base, b_base, b_base, b_base, b_base, b_base, b_base];
                SPMEM::write_vector_16(0x10, &input2);
                RSP::start_running(0);

                // Compute on the CPU
                for b_offset in 0..STEPS_PER_RSP {
                    let b = b_base + b_offset as u16;
                    for i in 0..8 {
                        let product = ((a_base + i as u16) as i16 as i32) * (b as i16 as i32);
                        let temp = ((product as i64) << 1) + 0x8000;
                        cpu_out_data[b_offset][0][i] = if (temp > 0) && ((temp & !0x7FFFFFFF) != 0) { 0x7FFF } else { (temp >> 16) as u16 };
                        cpu_out_data[b_offset][1][i] = (temp >> 32) as u16;
                        cpu_out_data[b_offset][2][i] = (temp >> 16) as u16;
                        cpu_out_data[b_offset][3][i] = temp as u16;
                    }
                }
                // Wait until RSP is finished and get its results
                RSP::wait_until_rsp_is_halted_and_dma_completed();
                for b_offset in 0..STEPS_PER_RSP {
                    let b = b_base + b_offset as u16;
                    let rsp_result = unsafe { MemoryMap::uncached(&rsp_out_data[b_offset][0]).read_volatile() };
                    let rsp_acc_high = unsafe { MemoryMap::uncached(&rsp_out_data[b_offset][1]).read_volatile() };
                    let rsp_acc_mid = unsafe { MemoryMap::uncached(&rsp_out_data[b_offset][2]).read_volatile() };
                    let rsp_acc_low = unsafe { MemoryMap::uncached(&rsp_out_data[b_offset][3]).read_volatile() };
                    for column in 0..8 {
                        soft_assert_eq2(rsp_result[column], cpu_out_data[b_offset][0][column], || format!("Result vector for inputs 0x{:x} and 0x{:x}", (a_base as usize + column) as i16, b))?;
                        soft_assert_eq2(rsp_acc_high[column], cpu_out_data[b_offset][1][column], || format!("Acc[32..48] for inputs 0x{:x} and 0x{:x}", (a_base as usize + column) as i16, b))?;
                        soft_assert_eq2(rsp_acc_mid[column], cpu_out_data[b_offset][2][column], || format!("Acc[16..32] for inputs 0x{:x} and 0x{:x}", (a_base as usize + column) as i16, b))?;
                        soft_assert_eq2(rsp_acc_low[column], cpu_out_data[b_offset][3][column], || format!("Acc[0..16] for inputs 0x{:x} and 0x{:x}", (a_base as usize + column) as i16, b))?;
                    }
                }
            }
        }


        Ok(())
    }
}
