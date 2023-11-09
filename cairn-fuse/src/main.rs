// Based on the simple.rs implementation in the fuser repo
#![allow(clippy::needless_return)]
#![allow(clippy::unnecessary_cast)] // libc::S_* are u16 or u32 depending on the platform

use clap::{crate_version, Arg, Command};
use fuser::consts::FOPEN_DIRECT_IO;
#[cfg(feature = "abi-7-26")]
use fuser::consts::FUSE_HANDLE_KILLPRIV;
#[cfg(feature = "abi-7-31")]
use fuser::consts::FUSE_WRITE_KILL_PRIV;
use fuser::TimeOrNow::Now;
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, ReplyXattr, Request, TimeOrNow,
    FUSE_ROOT_ID,
};
#[cfg(feature = "abi-7-26")]
use log::info;
use log::{debug, warn};
use log::{error, LevelFilter};
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::raw::c_int;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs as ufs;
use std::os::unix::fs::FileExt;
#[cfg(target_os = "linux")]
use std::os::unix::io::IntoRawFd;
use std::os::unix::prelude::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};
use walkdir::WalkDir;

const BLOCK_SIZE: u64 = 512;
const MAX_NAME_LENGTH: u32 = 255;
const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024 * 1024;

// Top two file handle bits are used to store permissions
// Note: This isn't safe, since the client can modify those bits. However, this implementation
// is just a toy
const FILE_HANDLE_READ_BIT: u64 = 1 << 63;
const FILE_HANDLE_WRITE_BIT: u64 = 1 << 62;

const FMODE_EXEC: i32 = 0x20;

type Inode = u64;

type DirectoryDescriptor = BTreeMap<Vec<u8>, (Inode, FileKind)>;

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
enum FileKind {
    File,
    Directory,
    Symlink,
}

impl From<FileKind> for fuser::FileType {
    fn from(kind: FileKind) -> Self {
        match kind {
            FileKind::File => fuser::FileType::RegularFile,
            FileKind::Directory => fuser::FileType::Directory,
            FileKind::Symlink => fuser::FileType::Symlink,
        }
    }
}

// #[derive(Debug)]
// enum XattrNamespace {
//     Security,
//     System,
//     Trusted,
//     User,
// }
//
// fn parse_xattr_namespace(key: &[u8]) -> Result<XattrNamespace, c_int> {
//     let user = b"user.";
//     if key.len() < user.len() {
//         return Err(libc::ENOTSUP);
//     }
//     if key[..user.len()].eq(user) {
//         return Ok(XattrNamespace::User);
//     }
//
//     let system = b"system.";
//     if key.len() < system.len() {
//         return Err(libc::ENOTSUP);
//     }
//     if key[..system.len()].eq(system) {
//         return Ok(XattrNamespace::System);
//     }
//
//     let trusted = b"trusted.";
//     if key.len() < trusted.len() {
//         return Err(libc::ENOTSUP);
//     }
//     if key[..trusted.len()].eq(trusted) {
//         return Ok(XattrNamespace::Trusted);
//     }
//
//     let security = b"security";
//     if key.len() < security.len() {
//         return Err(libc::ENOTSUP);
//     }
//     if key[..security.len()].eq(security) {
//         return Ok(XattrNamespace::Security);
//     }
//
//     return Err(libc::ENOTSUP);
// }
//
fn clear_suid_sgid(attr: &mut InodeAttributes) {
    attr.mode &= !libc::S_ISUID as u16;
    // SGID is only suppose to be cleared if XGRP is set
    if attr.mode & libc::S_IXGRP as u16 != 0 {
        attr.mode &= !libc::S_ISGID as u16;
    }
}

fn creation_gid(parent: &InodeAttributes, gid: u32) -> u32 {
    if parent.mode & libc::S_ISGID as u16 != 0 {
        return parent.gid;
    }

    gid
}
//
// fn xattr_access_check(
//     key: &[u8],
//     access_mask: i32,
//     inode_attrs: &InodeAttributes,
//     request: &Request<'_>,
// ) -> Result<(), c_int> {
//     match parse_xattr_namespace(key)? {
//         XattrNamespace::Security => {
//             if access_mask != libc::R_OK && request.uid() != 0 {
//                 return Err(libc::EPERM);
//             }
//         }
//         XattrNamespace::Trusted => {
//             if request.uid() != 0 {
//                 return Err(libc::EPERM);
//             }
//         }
//         XattrNamespace::System => {
//             if key.eq(b"system.posix_acl_access") {
//                 if !check_access(
//                     inode_attrs.uid,
//                     inode_attrs.gid,
//                     inode_attrs.mode,
//                     request.uid(),
//                     request.gid(),
//                     access_mask,
//                 ) {
//                     return Err(libc::EPERM);
//                 }
//             } else if request.uid() != 0 {
//                 return Err(libc::EPERM);
//             }
//         }
//         XattrNamespace::User => {
//             if !check_access(
//                 inode_attrs.uid,
//                 inode_attrs.gid,
//                 inode_attrs.mode,
//                 request.uid(),
//                 request.gid(),
//                 access_mask,
//             ) {
//                 return Err(libc::EPERM);
//             }
//         }
//     }
//
//     Ok(())
// }

fn time_now() -> (i64, u32) {
    time_from_system_time(&SystemTime::now())
}

fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}

fn time_from_system_time(system_time: &SystemTime) -> (i64, u32) {
    // Convert to signed 64-bit time with epoch at 0
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(before_epoch_error) => (
            -(before_epoch_error.duration().as_secs() as i64),
            before_epoch_error.duration().subsec_nanos(),
        ),
    }
}

struct InodeAttributes {
    pub metadata: fs::Metadata,
    pub real_path: String,
}

impl From<InodeAttributes> for fuser::FileAttr {
    fn from(attrs: InodeAttributes) -> Self {
        let last_accessed = if let Ok(system_time) = attrs.metadata.accessed() {
            time_from_system_time(&system_time)
        } else {
            time_now()
        };
        let (last_modified, last_metadata_changed) =
            if let Ok(system_time) = attrs.metadata.modified() {
                (
                    time_from_system_time(&system_time),
                    time_from_system_time(&system_time),
                )
            } else {
                (time_now(), time_now())
            };

        let filetype = attrs.metadata.file_type();
        let kind = if filetype.is_file() {
            FileKind::File
        } else if filetype.is_dir() {
            FileKind::Directory
        } else if filetype.is_symlink() {
            FileKind::Symlink
        } else {
            unimplemented!();
        };

        fuser::FileAttr {
            ino: attrs.metadata.ino(),
            size: attrs.metadata.len(),
            blocks: attrs.metadata.blocks(),
            atime: system_time_from_time(last_accessed.0, last_accessed.1),
            mtime: system_time_from_time(last_modified.0, last_modified.1),
            ctime: system_time_from_time(last_metadata_changed.0, last_metadata_changed.1),
            crtime: SystemTime::UNIX_EPOCH,
            kind: kind.into(),
            perm: attrs.metadata.permissions().mode() as u16,
            nlink: attrs.metadata.nlink() as u32,
            uid: attrs.metadata.uid(),
            gid: attrs.metadata.gid(),
            rdev: attrs.metadata.rdev() as u32,
            blksize: attrs.metadata.blksize() as u32,
            flags: 0,
        }
    }
}

// Stores inode metadata data in "$data_dir/inodes" and file contents in "$data_dir/contents"
// Directory data is stored in the file's contents, as a serialized DirectoryDescriptor
struct TracerFS {
    root: String,
    data_dir: String,
    context_map: BTreeMap<u64, InodeAttributes>,
}

impl TracerFS {
    fn new(root: String, data_dir: String) -> TracerFS {
        {
            TracerFS {
                root,
                data_dir,
                context_map: BTreeMap::new(),
            }
        }
    }

    fn lookup_name(&self, parent: u64, name: &OsStr) -> Result<InodeAttributes, c_int> {
        let parent_context = self.context_map.get(&parent).unwrap();
        let parent_path = Path::new(&parent_context.real_path);
        let path = parent_path.join(name);
        let metadata = fs::metadata(path);
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                Ok(context)
            }
            Err(e) => Err(e.raw_os_error().unwrap_or(libc::EIO)),
        }
    }

    // fn creation_mode(&self, mode: u32) -> u16 {
    //     (mode & !(libc::S_ISUID | libc::S_ISGID) as u32) as u16
    // }

    // fn allocate_next_inode(&self) -> Inode {
    //     let path = Path::new(&self.data_dir).join("superblock");
    //     let current_inode = if let Ok(file) = File::open(&path) {
    //         bincode::deserialize_from(file).unwrap()
    //     } else {
    //         fuser::FUSE_ROOT_ID
    //     };

    //     let file = OpenOptions::new()
    //         .write(true)
    //         .create(true)
    //         .truncate(true)
    //         .open(&path)
    //         .unwrap();
    //     bincode::serialize_into(file, &(current_inode + 1)).unwrap();

    //     current_inode + 1
    // }

    // fn allocate_next_file_handle(&self, read: bool, write: bool) -> u64 {
    //     let mut fh = self.next_file_handle.fetch_add(1, Ordering::SeqCst);
    //     // Assert that we haven't run out of file handles
    //     assert!(fh < FILE_HANDLE_WRITE_BIT && fh < FILE_HANDLE_READ_BIT);
    //     if read {
    //         fh |= FILE_HANDLE_READ_BIT;
    //     }
    //     if write {
    //         fh |= FILE_HANDLE_WRITE_BIT;
    //     }

    //     fh
    // }

    // fn check_file_handle_read(&self, file_handle: u64) -> bool {
    //     (file_handle & FILE_HANDLE_READ_BIT) != 0
    // }

    // fn check_file_handle_write(&self, file_handle: u64) -> bool {
    //     (file_handle & FILE_HANDLE_WRITE_BIT) != 0
    // }

    // fn content_path(&self, inode: Inode) -> PathBuf {
    //     Path::new(&self.data_dir)
    //         .join("contents")
    //         .join(inode.to_string())
    // }

    // fn get_directory_content(&self, inode: Inode) -> Result<DirectoryDescriptor, c_int> {
    //     let path = Path::new(&self.data_dir)
    //         .join("contents")
    //         .join(inode.to_string());
    //     if let Ok(file) = File::open(path) {
    //         Ok(bincode::deserialize_from(file).unwrap())
    //     } else {
    //         Err(libc::ENOENT)
    //     }
    // }

    // fn write_directory_content(&self, inode: Inode, entries: DirectoryDescriptor) {
    //     let path = Path::new(&self.data_dir)
    //         .join("contents")
    //         .join(inode.to_string());
    //     let file = OpenOptions::new()
    //         .write(true)
    //         .create(true)
    //         .truncate(true)
    //         .open(path)
    //         .unwrap();
    //     bincode::serialize_into(file, &entries).unwrap();
    // }

    // fn get_inode(&self, inode: Inode) -> Result<InodeAttributes, c_int> {
    //     let path = Path::new(&self.data_dir)
    //         .join("inodes")
    //         .join(inode.to_string());
    //     if let Ok(file) = File::open(path) {
    //         Ok(bincode::deserialize_from(file).unwrap())
    //     } else {
    //         Err(libc::ENOENT)
    //     }
    // }

    // fn write_inode(&self, inode: &InodeAttributes) {
    //     let path = Path::new(&self.data_dir)
    //         .join("inodes")
    //         .join(inode.inode.to_string());
    //     let file = OpenOptions::new()
    //         .write(true)
    //         .create(true)
    //         .truncate(true)
    //         .open(path)
    //         .unwrap();
    //     bincode::serialize_into(file, inode).unwrap();
    // }

    // // Check whether a file should be removed from storage. Should be called after decrementing
    // // the link count, or closing a file handle
    // fn gc_inode(&self, inode: &InodeAttributes) -> bool {
    //     if inode.hardlinks == 0 && inode.open_file_handles == 0 {
    //         let inode_path = Path::new(&self.data_dir)
    //             .join("inodes")
    //             .join(inode.inode.to_string());
    //         fs::remove_file(inode_path).unwrap();
    //         let content_path = Path::new(&self.data_dir)
    //             .join("contents")
    //             .join(inode.inode.to_string());
    //         fs::remove_file(content_path).unwrap();

    //         return true;
    //     }

    //     return false;
    // }

    // fn truncate(
    //     &self,
    //     inode: Inode,
    //     new_length: u64,
    //     uid: u32,
    //     gid: u32,
    // ) -> Result<InodeAttributes, c_int> {
    //     if new_length > MAX_FILE_SIZE {
    //         return Err(libc::EFBIG);
    //     }

    //     let mut attrs = self.get_inode(inode)?;

    //     if !check_access(attrs.uid, attrs.gid, attrs.mode, uid, gid, libc::W_OK) {
    //         return Err(libc::EACCES);
    //     }

    //     let path = self.content_path(inode);
    //     let file = OpenOptions::new().write(true).open(path).unwrap();
    //     file.set_len(new_length).unwrap();

    //     attrs.size = new_length;
    //     attrs.last_metadata_changed = time_now();
    //     attrs.last_modified = time_now();

    //     // Clear SETUID & SETGID on truncate
    //     clear_suid_sgid(&mut attrs);

    //     self.write_inode(&attrs);

    //     Ok(attrs)
    // }

    // fn get_path(&self, inode: Inode, name: &OsStr) -> PathBuf {
    //     Path::new(&self.get_inode(inode).unwrap().real_path).join(name)
    // }

    // fn lookup_name(&self, parent: u64, name: &OsStr) -> Result<fs::Metadata, c_int> {
    //     fs::metadata(self.get_path(parent, name)).map_err(|e| e.raw_os_error().unwrap_or(libc::EIO))
    //     // let entries = self.get_directory_content(parent)?;
    //     // if let Some((inode, _)) = entries.get(name.as_bytes()) {
    //     //     return self.get_inode(*inode);
    //     // } else {
    //     //     return Err(libc::ENOENT);
    //     // }
    // }

    // fn insert_link(
    //     &self,
    //     req: &Request,
    //     parent: u64,
    //     name: &OsStr,
    //     inode: u64,
    //     kind: FileKind,
    // ) -> Result<(), c_int> {
    //     if self.lookup_name(parent, name).is_ok() {
    //         return Err(libc::EEXIST);
    //     }

    //     let mut parent_attrs = self.get_inode(parent)?;

    //     if !check_access(
    //         parent_attrs.uid,
    //         parent_attrs.gid,
    //         parent_attrs.mode,
    //         req.uid(),
    //         req.gid(),
    //         libc::W_OK,
    //     ) {
    //         return Err(libc::EACCES);
    //     }
    //     parent_attrs.last_modified = time_now();
    //     parent_attrs.last_metadata_changed = time_now();
    //     self.write_inode(&parent_attrs);

    //     let mut entries = self.get_directory_content(parent).unwrap();
    //     entries.insert(name.as_bytes().to_vec(), (inode, kind));
    //     self.write_directory_content(parent, entries);

    //     Ok(())
    // }
}

impl Filesystem for TracerFS {
    fn init(
        &mut self,
        _req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), c_int> {
        for entry in WalkDir::new(&self.root).into_iter().filter_map(|e| e.ok()) {
            let metadata = entry.metadata().unwrap();
            let real_path = entry.path().to_str().unwrap().to_string();
            let inode = metadata.ino();
            let context = InodeAttributes {
                metadata,
                real_path,
            };
            self.context_map.insert(inode, context);
        }

        Ok(())
    }

    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup(parent={}, name={:?})", parent, name);

        match self.lookup_name(parent, name) {
            Ok(attrs) => {
                reply.entry(&Duration::new(0, 0), &attrs.into(), 0);
            }
            Err(e) => {
                reply.error(e);
            }
        }
    }

    fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {
        debug!("forget(ino={}, nlookup={})", _ino, _nlookup);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr(ino={})", ino);

        match self.context_map.get(&ino) {
            Some(attrs) => {
                reply.attr(&Duration::new(0, 0), &(*attrs).into());
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn setattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        // get attrs and handle it properly
        let mut attrs = match self.context_map.get(&ino) {
            Some(attrs) => attrs,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        if let Some(mode) = mode {
            debug!("chmod() called with {:?}, {:o}", ino, mode);
            if req.uid() != 0 && req.uid() != attrs.metadata.uid() {
                reply.error(libc::EPERM);
                return;
            }

            attrs.metadata.permissions().set_mode(mode);
            utime::set_file_times(
                &attrs.real_path,
                time_from_system_time(&attrs.metadata.accessed().unwrap()).0,
                time_now().0,
            )
            .unwrap();

            self.context_map.insert(ino, *attrs);
            reply.attr(&Duration::new(0, 0), &(*attrs).into());
            return;
        }

        if uid.is_some() || gid.is_some() {
            debug!("chown() called with {:?} {:?} {:?}", inode, uid, gid);

        }

    }
}

pub fn check_access(
    file_uid: u32,
    file_gid: u32,
    file_mode: u16,
    uid: u32,
    gid: u32,
    mut access_mask: i32,
) -> bool {
    // F_OK tests for existence of file
    if access_mask == libc::F_OK {
        return true;
    }
    let file_mode = i32::from(file_mode);

    // root is allowed to read & write anything
    if uid == 0 {
        // root only allowed to exec if one of the X bits is set
        access_mask &= libc::X_OK;
        access_mask -= access_mask & (file_mode >> 6);
        access_mask -= access_mask & (file_mode >> 3);
        access_mask -= access_mask & file_mode;
        return access_mask == 0;
    }

    if uid == file_uid {
        access_mask -= access_mask & (file_mode >> 6);
    } else if gid == file_gid {
        access_mask -= access_mask & (file_mode >> 3);
    } else {
        access_mask -= access_mask & file_mode;
    }

    return access_mask == 0;
}

fn as_file_kind(mut mode: u32) -> FileKind {
    mode &= libc::S_IFMT as u32;

    if mode == libc::S_IFREG as u32 {
        return FileKind::File;
    } else if mode == libc::S_IFLNK as u32 {
        return FileKind::Symlink;
    } else if mode == libc::S_IFDIR as u32 {
        return FileKind::Directory;
    } else {
        unimplemented!("{}", mode);
    }
}

fn get_groups(pid: u32) -> Vec<u32> {
    #[cfg(not(target_os = "macos"))]
    {
        let path = format!("/proc/{pid}/task/{pid}/status");
        let file = File::open(path).unwrap();
        for line in BufReader::new(file).lines() {
            let line = line.unwrap();
            if line.starts_with("Groups:") {
                return line["Groups: ".len()..]
                    .split(' ')
                    .filter(|x| !x.trim().is_empty())
                    .map(|x| x.parse::<u32>().unwrap())
                    .collect();
            }
        }
    }

    vec![]
}

fn fuse_allow_other_enabled() -> io::Result<bool> {
    let file = File::open("/etc/fuse.conf")?;
    for line in BufReader::new(file).lines() {
        if line?.trim_start().starts_with("user_allow_other") {
            return Ok(true);
        }
    }
    Ok(false)
}

// fn main() {
//     let matches = Command::new("Fuser")
//         .version(crate_version!())
//         .author("Christopher Berner")
//         .arg(
//             Arg::new("data-dir")
//                 .long("data-dir")
//                 .value_name("DIR")
//                 .default_value("/tmp/fuser")
//                 .help("Set local directory used to store data")
//                 .takes_value(true),
//         )
//         .arg(
//             Arg::new("mount-point")
//                 .long("mount-point")
//                 .value_name("MOUNT_POINT")
//                 .default_value("")
//                 .help("Act as a client, and mount FUSE at given path")
//                 .takes_value(true),
//         )
//         .arg(
//             Arg::new("direct-io")
//                 .long("direct-io")
//                 .requires("mount-point")
//                 .help("Mount FUSE with direct IO"),
//         )
//         .arg(Arg::new("fsck").long("fsck").help("Run a filesystem check"))
//         .arg(
//             Arg::new("suid")
//                 .long("suid")
//                 .help("Enable setuid support when run as root"),
//         )
//         .arg(
//             Arg::new("v")
//                 .short('v')
//                 .multiple_occurrences(true)
//                 .help("Sets the level of verbosity"),
//         )
//         .get_matches();
//
//     let verbosity: u64 = matches.occurrences_of("v");
//     let log_level = match verbosity {
//         0 => LevelFilter::Error,
//         1 => LevelFilter::Warn,
//         2 => LevelFilter::Info,
//         3 => LevelFilter::Debug,
//         _ => LevelFilter::Trace,
//     };
//     env_logger::builder()
//         .format_timestamp_nanos()
//         .filter_level(log_level)
//         .init();
//
//     let mut options = vec![MountOption::FSName("fuser".to_string())];
//
//     #[cfg(feature = "abi-7-26")]
//     {
//         if matches.is_present("suid") {
//             info!("setuid bit support enabled");
//             options.push(MountOption::Suid);
//         } else {
//             options.push(MountOption::AutoUnmount);
//         }
//     }
//     #[cfg(not(feature = "abi-7-26"))]
//     {
//         options.push(MountOption::AutoUnmount);
//     }
//     if let Ok(enabled) = fuse_allow_other_enabled() {
//         if enabled {
//             options.push(MountOption::AllowOther);
//         }
//     } else {
//         eprintln!("Unable to read /etc/fuse.conf");
//     }
//
//     let data_dir: String = matches.value_of("data-dir").unwrap_or_default().to_string();
//
//     let mountpoint: String = matches
//         .value_of("mount-point")
//         .unwrap_or_default()
//         .to_string();
//
//     let result = fuser::mount2(
//         TracerFS::new(
//             data_dir,
//             matches.is_present("direct-io"),
//             matches.is_present("suid"),
//         ),
//         mountpoint,
//         &options,
//     );
//     if let Err(e) = result {
//         // Return a special error code for permission denied, which usually indicates that
//         // "user_allow_other" is missing from /etc/fuse.conf
//         if e.kind() == ErrorKind::PermissionDenied {
//             error!("{}", e.to_string());
//             std::process::exit(2);
//         }
//     }
// }

fn main() {
    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version(crate_version!())
        .about("Filesystem implementation for tracing I/O operations for forward build systems")
        .arg(
            Arg::new("root")
                .help("Root directory for the filesystem")
                .required(true),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .value_name("DIR")
                .default_value("/tmp/cairn")
                .help("Set local directory used to store data")
                .required(true),
        )
        .arg(
            Arg::new("mount-point")
                .help("Mountpoint for the filesystem")
                .required(true),
        )
        .arg(Arg::new("v").short('v').help("Sets the level of verbosity"))
        .get_matches();

    let verbosity = matches.get_one::<u64>("v").unwrap();
    let log_level = match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::builder()
        .format_timestamp_nanos()
        .filter_level(log_level)
        .init();

    let root = matches.get_one::<String>("root").unwrap().to_string();
    let mountpoint = matches.get_one::<String>("mount-point").unwrap();
    let data_dir = matches.get_one::<String>("data-dir").unwrap();
    let options = vec![MountOption::FSName("cairn-fuse".to_string())];

    let result = fuser::mount2(TracerFS::new(root), mountpoint, &options);

    if let Err(e) = result {
        error!("Error mounting filesystem: {}", e);
    }
}
