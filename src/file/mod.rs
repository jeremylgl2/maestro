//! This module handles filesystems. Every filesystems are unified by the Virtual FileSystem (VFS).
//! The root filesystem is passed to the kernel as an argument when booting. Other filesystems are
//! mounted into subdirectories.

pub mod ext2;
pub mod file_descriptor;
pub mod filesystem;
pub mod mountpoint;
pub mod path;

use core::mem::MaybeUninit;
use crate::device::DeviceType;
use crate::errno::Errno;
use crate::errno;
use crate::file::mountpoint::MountPoint;
use crate::time::Timestamp;
use crate::time;
use crate::util::FailableClone;
use crate::util::container::hashmap::HashMap;
use crate::util::container::string::String;
use crate::util::container::vec::Vec;
use crate::util::lock::mutex::Mutex;
use crate::util::ptr::SharedPtr;
use crate::util::ptr::WeakPtr;
use path::Path;

/// Type representing a user ID.
pub type Uid = u16;
/// Type representing a group ID.
pub type Gid = u16;
/// Type representing a file mode.
pub type Mode = u16;
/// Type representing an inode ID.
pub type INode = u32;

/// User: Read, Write and Execute.
pub const S_IRWXU: Mode = 00700;
/// User: Read.
pub const S_IRUSR: Mode = 00400;
/// User: Write.
pub const S_IWUSR: Mode = 00200;
/// User: Execute.
pub const S_IXUSR: Mode = 00100;
/// Group: Read, Write and Execute.
pub const S_IRWXG: Mode = 00070;
/// Group: Read.
pub const S_IRGRP: Mode = 00040;
/// Group: Write.
pub const S_IWGRP: Mode = 00020;
/// Group: Execute.
pub const S_IXGRP: Mode = 00010;
/// Other: Read, Write and Execute.
pub const S_IRWXO: Mode = 00007;
/// Other: Read.
pub const S_IROTH: Mode = 00004;
/// Other: Write.
pub const S_IWOTH: Mode = 00002;
/// Other: Execute.
pub const S_IXOTH: Mode = 00001;
/// Setuid.
pub const S_ISUID: Mode = 04000;
/// Setgid.
pub const S_ISGID: Mode = 02000;
/// Sticky bit.
pub const S_ISVTX: Mode = 01000;

/// The number of buckets in the hash map storing a directory's subfiles.
pub const SUBFILES_HASHMAP_BUCKETS: usize = 16;

/// The size of the files pool.
pub const FILES_POOL_SIZE: usize = 1024;
/// The upper bount for the file accesses counter.
pub const ACCESSES_UPPER_BOUND: usize = 128;

/// Enumeration representing the different file types.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
	/// A regular file storing data.
	Regular,
	/// A directory, containing other files.
	Directory,
	/// A symbolic link, pointing to another file.
	Link,
	/// A named pipe.
	FIFO,
	/// A Unix domain socket.
	Socket,
	/// A Block device file.
	BlockDevice,
	/// A Character device file.
	CharDevice,
}

// TODO Use a structure wrapping files for lazy allocations
/// Structure representing a file.
pub struct File {
	/// The name of the file.
	name: String,

	/// Pointer to the parent file.
	parent: Option<WeakPtr<File>>,

	/// The size of the file in bytes.
	size: usize,
	/// The type of the file.
	file_type: FileType,

	/// The ID of the owner user.
	uid: Uid,
	/// The ID of the owner group.
	gid: Gid,
	/// The mode of the file.
	mode: Mode,

	/// The inode. None means that the file is not stored on any filesystem.
	inode: Option::<INode>,

	/// Timestamp of the last modification of the metadata.
	ctime: Timestamp,
	/// Timestamp of the last modification of the file.
	mtime: Timestamp,
	/// Timestamp of the last access to the file.
	atime: Timestamp,

	/// The file's subfiles (applicable only if the file is a directory).
	subfiles: Option<HashMap<String, WeakPtr<File>>>,

	/// The link's target (applicable only if the file is a symbolic link).
	link_target: String,

	// TODO Store file data:
	// - FIFO: buffer (on ram only)
	// - Socket: buffer (on ram only)

	/// The device's major number (applicable only if the file is a block or char device)
	device_major: u32,
	/// The device's minor number (applicable only if the file is a block or char device)
	device_minor: u32,
}

impl File {
	/// Creates a new instance.
	/// `name` is the name of the file.
	/// `file_type` is the type of the file.
	/// `uid` is the id of the owner user.
	/// `gid` is the id of the owner group.
	/// `mode` is the permission of the file.
	pub fn new(name: String, file_type: FileType, uid: Uid, gid: Gid, mode: Mode)
		-> Result<Self, Errno> {
		let timestamp = time::get();

		let subfiles_hash_map = {
			if file_type == FileType::Directory {
				Some(HashMap::<String, WeakPtr<File>>::new(SUBFILES_HASHMAP_BUCKETS)?)
			} else {
				None
			}
		};

		Ok(Self {
			name: name,
			parent: None,

			size: 0,
			file_type: file_type,

			uid: uid,
			gid: gid,
			mode: mode,

			inode: None,

			ctime: timestamp,
			mtime: timestamp,
			atime: timestamp,

			subfiles: subfiles_hash_map,

			link_target: String::new(),

			device_major: 0,
			device_minor: 0,
		})
	}

	/// Returns the name of the file.
	pub fn get_name(&self) -> &String {
		&self.name
	}

	/// Returns a reference to the parent file.
	pub fn get_parent(&self) -> Option<&File> {
		self.parent.as_ref()?.get()
	}

	/// Returns the absolute path of the file.
	pub fn get_path(&self) -> Result<Path, Errno> {
		let name = self.get_name().failable_clone()?;

		if let Some(parent) = self.get_parent() {
			let mut path = parent.get_path()?;
			path.push(name)?;
			Ok(path)
		} else {
			let mut path = Path::root();
			path.push(name)?;
			Ok(path)
		}
	}

	/// Sets the file's parent.
	pub fn set_parent(&mut self, parent: Option<WeakPtr<File>>) {
		self.parent = parent;
	}

	/// Returns the size of the file in bytes.
	pub fn get_size(&self) -> usize {
		self.size
	}

	/// Returns the type of the file.
	pub fn get_file_type(&self) -> FileType {
		self.file_type
	}

	/// Returns the owner user ID.
	pub fn get_uid(&self) -> Uid {
		self.uid
	}

	/// Returns the owner group ID.
	pub fn get_gid(&self) -> Gid {
		self.gid
	}

	/// Returns the file's mode.
	pub fn get_mode(&self) -> Mode {
		self.mode
	}

	/// Returns the timestamp to the last modification of the file's metadata.
	pub fn get_ctime(&self) -> Timestamp {
		self.ctime
	}

	/// Returns the timestamp to the last modification to the file.
	pub fn get_mtime(&self) -> Timestamp {
		self.mtime
	}

	/// Returns the timestamp to the last access to the file.
	pub fn get_atime(&self) -> Timestamp {
		self.atime
	}

	/// Tells if the file can be read from by the given UID and GID.
	pub fn can_read(&self, uid: Uid, gid: Gid) -> bool {
		if self.uid == uid && self.mode & S_IRUSR != 0 {
			return true;
		}
		if self.gid == gid && self.mode & S_IRGRP != 0 {
			return true;
		}
		self.mode & S_IROTH != 0
	}

	/// Tells if the file can be written to by the given UID and GID.
	pub fn can_write(&self, uid: Uid, gid: Gid) -> bool {
		if self.uid == uid && self.mode & S_IWUSR != 0 {
			return true;
		}
		if self.gid == gid && self.mode & S_IWGRP != 0 {
			return true;
		}
		self.mode & S_IWOTH != 0
	}

	/// Tells if the file can be executed by the given UID and GID.
	pub fn can_execute(&self, uid: Uid, gid: Gid) -> bool {
		if self.uid == uid && self.mode & S_IXUSR != 0 {
			return true;
		}
		if self.gid == gid && self.mode & S_IXGRP != 0 {
			return true;
		}
		self.mode & S_IXOTH != 0
	}

	/// Tells whether the directory is empty or not. If the file is not a directory, the behaviour
	/// is undefined.
	pub fn is_empty_directory(&self) -> bool {
		debug_assert_eq!(self.file_type, FileType::Directory);
		self.subfiles.as_ref().unwrap().is_empty()
	}

	/// Adds the file `file` to the current file's subfiles. If the file isn't a directory, the
	/// behaviour is undefined.
	pub fn add_subfile(&mut self, file: WeakPtr<File>) -> Result<(), Errno> {
		debug_assert_eq!(self.file_type, FileType::Directory);
		let name = file.get().unwrap().get_name().failable_clone()?;
		self.subfiles.as_mut().unwrap().insert(name, file)?;
		Ok(())
	}

	/// Removes the file with name `name` from the current file's subfiles. If the file isn't a
	/// directory, the behaviour is undefined.
	pub fn remove_subfile(&mut self, name: String) {
		debug_assert_eq!(self.file_type, FileType::Directory);
		self.subfiles.as_mut().unwrap().remove(name);
	}

	/// Returns the symbolic link's target. If the file isn't a symbolic link, the behaviour is
	/// undefined.
	pub fn get_link_target(&self) -> &String {
		debug_assert_eq!(self.file_type, FileType::Link);
		&self.link_target
	}

	/// Sets the symbolic link's target. If the file isn't a symbolic link, the behaviour is
	/// undefined.
	pub fn set_link_target(&mut self, target: String) {
		debug_assert_eq!(self.file_type, FileType::Link);
		self.link_target = target;
	}

	/// Returns the device's major number. If the file isn't a block or char device, the behaviour
	/// is undefined.
	pub fn get_device_major(&self) -> u32 {
		debug_assert!(self.file_type == FileType::BlockDevice
			|| self.file_type == FileType::CharDevice);
		self.device_major
	}

	/// Sets the device's major number. If the file isn't a block or char device, the behaviour
	/// is undefined.
	pub fn set_device_major(&mut self, major: u32) {
		debug_assert!(self.file_type == FileType::BlockDevice
			|| self.file_type == FileType::CharDevice);
		self.device_major = major;
	}

	/// Returns the device's minor number. If the file isn't a block or char device, the behaviour
	/// is undefined.
	pub fn get_device_minor(&self) -> u32 {
		debug_assert!(self.file_type == FileType::BlockDevice
			|| self.file_type == FileType::CharDevice);
		self.device_minor
	}

	/// Sets the device's minor number. If the file isn't a block or char device, the behaviour
	/// is undefined.
	pub fn set_device_minor(&mut self, minor: u32) {
		debug_assert!(self.file_type == FileType::BlockDevice
			|| self.file_type == FileType::CharDevice);
		self.device_minor = minor;
	}

	/// Synchronizes the file with the device.
	pub fn sync(&self) {
		if self.inode.is_some() {
			// TODO
		}
	}

	/// Unlinks the current file.
	pub fn unlink(&mut self) {
		// TODO
	}
}

impl Drop for File {
	fn drop(&mut self) {
		self.sync();
	}
}

/// The access counter allows to count the relative number of accesses count on a file.
pub struct AccessCounter {
	/// The number of accesses to the file relative to the previous file in the pool.
	/// This number is limited by `ACCESSES_UPPER_BOUND`.
	accesses_count: usize,
}

///	Cache storing files in memory. This cache allows to speedup accesses to the disk. It is
/// synchronized with the disk when necessary.
pub struct FCache {
	/// A pointer to the root mount point.
	root_mount: SharedPtr<MountPoint>,

	/// A fixed-size pool storing files, sorted by path.
	files_pool: Vec<File>,
	/// A pool of the same size as the files pool, storing approximate relative accesses count for
	/// each files.
	/// The element at an index is associated to the element in the files pool at the same index.
	accesses_pool: Vec<AccessCounter>,
}

impl FCache {
	/// Creates a new instance with the given major and minor for the root device.
	pub fn new(root_device_type: DeviceType, root_major: u32, root_minor: u32)
		-> Result<Self, Errno> {
		let root_mount = MountPoint::new(root_device_type, root_major, root_minor, Path::root())?;
		let shared_ptr = mountpoint::register_mountpoint(root_mount)?;

		Ok(Self {
			root_mount: shared_ptr,

			files_pool: Vec::<File>::with_capacity(FILES_POOL_SIZE)?,
			accesses_pool: Vec::<AccessCounter>::with_capacity(FILES_POOL_SIZE)?,
		})
	}

	/// Loads the file with the given path `path`. If the file is already loaded, the behaviour is
	/// undefined.
	fn load_file(&mut self, _path: &Path) {
		let len = self.files_pool.len();
		if len >= FILES_POOL_SIZE {
			self.files_pool.pop();
			self.accesses_pool.pop();
		}

		// TODO Push file
	}

	/// Adds the file `file` to the VFS. The file will be located into the directory at path
	/// `path`.
	/// The directory must exist. If an error happens, the function returns an Err with the
	/// appropriate Errno.
	/// If the path is relative, the function starts from the root.
	/// If the file isn't present in the pool, the function shall load it.
	pub fn create_file(&mut self, _path: &Path, _file: File) -> Result<(), Errno> {
		// TODO

		Err(errno::ENOMEM)
	}

	// TODO Function to list files at root

	/// Returns the file at the root of the VFS with name `name`.
	pub fn get_root_file(&mut self, _name: String) -> Result<SharedPtr<File>, Errno> {
		// TODO

		Err(errno::ENOMEM)
	}

	/// Returns a reference to the file at path `path`. If the file doesn't exist, the function
	/// returns None.
	/// If the path is relative, the function starts from the root.
	/// If the file isn't present in the pool, the function shall load it.
	pub fn get_file_from_path(&mut self, _path: &Path) -> Result<SharedPtr<File>, Errno> {
		// TODO

		Err(errno::ENOMEM)
	}

	// TODO File remove
}

/// The instance of the file cache.
static mut FILES_CACHE: MaybeUninit<Mutex<FCache>> = MaybeUninit::uninit();

/// Registers the filesystems that are implemented inside of the kernel itself.
fn register_default_fs() -> Result<(), Errno> {
	filesystem::register(ext2::Ext2FsType {})?;

	Ok(())
}

/// Initializes files management.
/// `root_device_type` is the type of the root device file. If not a device, the behaviour is
/// undefined.
/// `root_major` is the major number of the device at the root of the VFS.
/// `root_minor` is the minor number of the device at the root of the VFS.
pub fn init(root_device_type: DeviceType, root_major: u32, root_minor: u32) -> Result<(), Errno> {
	register_default_fs()?;

	let cache = FCache::new(root_device_type, root_major, root_minor)?;
	unsafe {
		FILES_CACHE = MaybeUninit::new(Mutex::new(cache));
	}

	Ok(())
}

/// Returns a mutable reference to the file cache.
pub fn get_files_cache() -> &'static mut Mutex<FCache> {
	unsafe { // Safe because using Mutex
		FILES_CACHE.assume_init_mut()
	}
}

/// Removes the file at path `path` and its subfiles recursively if it's a directory.
/// If relative, the path is taken from the root.
pub fn remove_recursive(_path: &Path) {
	// TODO
}

/// Creates the directories necessary to reach path `path`. On success, the function returns
/// the number of created directories (without the directories that already existed).
/// If relative, the path is taken from the root.
pub fn create_dirs(_path: &Path) -> Result<usize, Errno> {
	// TODO

	Err(errno::ENOMEM)
}