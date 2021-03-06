/// This module implements debugging tools.

use core::ffi::c_void;
use core::mem::size_of;
use crate::elf;
use crate::memory;
use crate::multiboot;

/// Returns the value into the specified register.
#[macro_export]
macro_rules! register_get {
	($reg:expr) => {{
		let mut val: u32;
		llvm_asm!(concat!("mov %", $reg, ", %eax") : "={eax}"(val)); // TODO Use new syntax

		val
	}};
}

/// Prints, in hexadecimal, the content of the memory at the given location `ptr`, with the given
/// size `n` in bytes.
pub unsafe fn print_memory(ptr: *const c_void, n: usize) {
	let mut i = 0;
	while i < n {
		crate::print!("{:#08x}  ", ptr as usize + i);

		let mut j = 0;
		while j < 16 && i + j < n {
			crate::print!("{:02x} ", *(((ptr as usize) + (i + j)) as *const u8));
			j += 1;
		}

		crate::print!(" |");

		j = 0;
		while j < 16 && i + j < n {
			let v = *(((ptr as usize) + (i + j)) as *const u8);
			let c = if v < 32 || v > 127 {
				'.'
			} else {
				v as char
			};
			crate::print!("{}", c);
			j += 1;
		}

		crate::println!("|");

		i += j;
	}
}

/// Prints the callstack in the current context, including symbol's name and address. `ebp` is
/// value of the `%ebp` register that is used as a starting point for printing. `max_depth` is the
/// maximum depth of the stack to print. If the stack is larger than the maximum depth, the
/// function shall print `...` at the end. If the callstack is empty, the function just prints
/// `Empty`.
pub fn print_callstack(ebp: *const u32, max_depth: usize) {
	crate::println!("--- Callstack ---");

	let boot_info = multiboot::get_boot_info();
	let mut i: usize = 0;
	let mut ebp_ = ebp;
	while ebp_ != 0 as *const u32 && i < max_depth {
		// TODO
		/*if !memory::vmem::is_mapped(memory::kern_to_virt(memory::cr3_get()), ebp_) {
			break;
		}*/
		let eip = unsafe { // Dereference of raw pointer
			*((ebp_ as usize + size_of::<usize>()) as *const u32) as *const c_void
		};
		if eip == (0 as *const _) {
			break;
		}

		if let Some(name) = elf::get_function_name(memory::kern_to_virt(boot_info.elf_sections),
			boot_info.elf_num as usize, boot_info.elf_shndx as usize,
			boot_info.elf_entsize as usize, eip) {
			crate::println!("{}: {:p} -> {}", i, eip, name);
		} else {
			crate::println!("{}: {:p} -> ???", i, eip);
		}

		unsafe {
			ebp_ = *(ebp_ as *const u32) as *const u32;
		}
		i += 1;
	}

	if i == 0 {
		crate::println!("Empty");
	} else if ebp_ != (0 as *const _) {
		crate::println!("...");
	}
}
