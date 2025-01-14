//! The initramfs is a tmpfs stored under the form of an archive. It is used as an initialization
//! environment which doesn't require disk accesses.

mod cpio;

use crate::device;
use crate::errno;
use crate::errno::Errno;
use crate::file;
use crate::file::path::Path;
use crate::file::perm::AccessProfile;
use crate::file::vfs;
use crate::file::File;
use crate::file::FileContent;
use crate::file::FileType;
use crate::util::container::hashmap::HashMap;
use crate::util::io::IO;
use crate::util::lock::Mutex;
use crate::util::ptr::arc::Arc;
use crate::util::TryClone;
use cpio::CPIOParser;

/// Updates the current parent used for the unpacking operation.
///
/// Arguments:
/// - `new` is the new parent's path.
/// - `stored` is the current parent. The tuple contains the path and the file.
/// - `retry` tells whether the function is called as a second try.
fn update_parent(
	new: &Path,
	stored: &mut Option<(Path, Arc<Mutex<File>>)>,
	retry: bool,
) -> Result<(), Errno> {
	// Getting the parent
	let result = match stored {
		Some((path, file)) if new.begins_with(path) => {
			let name = match new.try_clone()?.pop() {
				Some(name) => name,
				None => return Ok(()),
			};

			let mut f = file.lock();
			vfs::get_file_from_parent(&mut f, name, &AccessProfile::KERNEL, false)
		}

		Some(_) | None => vfs::get_file_from_path(new, &AccessProfile::KERNEL, false),
	};

	match result {
		Ok(file) => {
			*stored = Some((new.try_clone()?, file));
			Ok(())
		}

		// If the directory doesn't exist, create recursively
		Err(e) if !retry && e.as_int() == errno::ENOENT => {
			file::util::create_dirs(new)?;
			return update_parent(new, stored, true);
		}

		Err(e) => Err(e),
	}
}

// TODO Implement gzip decompression?
// FIXME The function doesn't work if files are not in the right order in the archive
/// Loads the initramsfs at the root of the VFS.
///
/// `data` is the slice of data representing the initramfs image.
pub fn load(data: &[u8]) -> Result<(), Errno> {
	// TODO Use a stack instead?
	// The stored parent directory
	let mut stored_parent: Option<(Path, Arc<Mutex<File>>)> = None;

	let cpio_parser = CPIOParser::new(data);
	for entry in cpio_parser {
		let hdr = entry.get_hdr();

		let mut parent_path = Path::from_str(entry.get_filename(), false)?;
		let Some(name) = parent_path.pop() else {
			continue;
		};

		let file_type = hdr.get_type();
		let content = match file_type {
			FileType::Regular => FileContent::Regular,
			FileType::Directory => FileContent::Directory(HashMap::new()),
			FileType::Link => FileContent::Link(entry.get_content().try_into()?),
			FileType::Fifo => FileContent::Fifo,
			FileType::Socket => FileContent::Socket,
			FileType::BlockDevice => FileContent::BlockDevice {
				major: device::id::major(hdr.c_rdev as _),
				minor: device::id::minor(hdr.c_rdev as _),
			},
			FileType::CharDevice => FileContent::CharDevice {
				major: device::id::major(hdr.c_rdev as _),
				minor: device::id::minor(hdr.c_rdev as _),
			},
		};

		// Change the parent directory if necessary
		let update = match &stored_parent {
			Some((path, _)) => path != &parent_path,
			None => true,
		};
		if update {
			update_parent(&parent_path, &mut stored_parent, false)?;
		}

		let parent_mutex = &stored_parent.as_ref().unwrap().1;
		let mut parent = parent_mutex.lock();

		// Create file
		let create_result = vfs::create_file(
			&mut parent,
			name,
			&AccessProfile::KERNEL,
			hdr.get_perms(),
			content,
		);
		let file_mutex = match create_result {
			Ok(file_mutex) => file_mutex,
			Err(e) if e.as_int() == errno::EEXIST => continue,
			Err(e) => return Err(e),
		};
		let mut file = file_mutex.lock();
		file.set_uid(hdr.c_uid);
		file.set_gid(hdr.c_gid);
		// Write content if the file is a regular file
		if file_type == FileType::Regular {
			let content = entry.get_content();
			file.write(0, content)?;
		}
		file.sync()?;
	}

	Ok(())
}
