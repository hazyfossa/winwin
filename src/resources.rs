use std::{marker::PhantomData, path::Path};

use windows::{
    Win32::{
        Foundation::{E_FAIL, E_POINTER, HANDLE, HMODULE, MEM_E_INVALID_SIZE},
        System::LibraryLoader::{
            BeginUpdateResourceW, EndUpdateResourceW, FindResourceW, LOAD_LIBRARY_AS_DATAFILE,
            LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW, LoadResource, LockResource,
            SizeofResource, UpdateResourceW,
        },
    },
    core::{Error, Free, PCWSTR, Result},
};

use crate::pass_text;

// TODO: common resource-ref type: kind; id; lang

pub const fn int_resource(id: u16) -> PCWSTR {
    PCWSTR::from_raw(id as usize as *const u16)
}

pub struct ResourceId {
    lpname: PCWSTR,
}

impl<'a> Into<ResourceId> for &'a str {
    fn into(self) -> ResourceId {
        ResourceId {
            lpname: pass_text(self),
        }
    }
}

impl Into<ResourceId> for u16 {
    fn into(self) -> ResourceId {
        ResourceId {
            lpname: int_resource(self),
        }
    }
}

pub struct RemoteExe {
    module: HMODULE,
}

impl RemoteExe {
    pub fn open(path: &str) -> Result<Self> {
        let module = unsafe {
            LoadLibraryExW(
                pass_text(path),
                None,
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )?
        };

        Ok(Self { module })
    }

    pub fn resources(&self) -> Resources<'_> {
        // Safety: `self` owns the module for it's lifetime
        unsafe { Resources::bind_lifetime(Some(self.module)) }
    }
}

impl Drop for RemoteExe {
    fn drop(&mut self) {
        unsafe { self.module.free() };
    }
}

pub fn current_resources() -> Resources<'static> {
    // Safety: windows won't clean up resources while
    // the current app is alive
    unsafe { Resources::bind_lifetime(None) }
}

#[derive(Clone, Copy)]
pub struct Resources<'a> {
    module_ref: Option<HMODULE>,
    _p: PhantomData<&'a ()>,
}

pub mod resource_kind {
    // TODO: Seal
    pub trait T {
        const ID: u16;
        type Repr<'a>;

        // TODO: propagate parse errors
        fn parse(bytes: &[u8]) -> Option<Self::Repr<'_>>;

        fn to_bytes<'a>(repr: Self::Repr<'a>) -> &'a [u8];
    }

    pub struct Data;
    impl T for Data {
        const ID: u16 = 10;
        type Repr<'a> = &'a [u8];

        fn parse(bytes: &[u8]) -> Option<Self::Repr<'_>> {
            Some(bytes)
        }

        fn to_bytes<'a>(repr: Self::Repr<'a>) -> &'a [u8] {
            repr
        }
    }
}

impl<'a> Resources<'a> {
    // Safety: this is equivalent to an unsafe (raw pointer -> reference) cast
    // caller needs to ensure `module` is alive for `'a`
    unsafe fn bind_lifetime(module: Option<HMODULE>) -> Self {
        Self {
            module_ref: module,
            _p: PhantomData,
        }
    }

    pub fn open<Kind: resource_kind::T>(
        &self,
        id: impl Into<ResourceId>,
    ) -> Result<Option<Kind::Repr<'a>>> {
        let lpname = id.into().lpname;
        let lptype = int_resource(Kind::ID);
        let hres = unsafe { FindResourceW(self.module_ref, lpname, lptype) };

        if hres.is_invalid() {
            return Ok(None);
        }

        let len = unsafe { SizeofResource(self.module_ref, hres) };
        let hmem = unsafe { LoadResource(self.module_ref, hres)? };
        let ptr = unsafe { LockResource(hmem) };

        if ptr.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }

        let bytes = if len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(ptr.cast(), len as usize) }
        };

        Kind::parse(bytes)
            .ok_or(Error::new(
                E_FAIL,
                "Parsing failure: Resource data is invalid",
            ))
            .map(Some)
    }
}

// MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT)
const LANG: u16 = 0x0000_0400;

pub struct ResourceUpdateTx {
    handle: HANDLE,
}

impl ResourceUpdateTx {
    pub fn open(path: &Path, allow_overwrite: bool) -> Result<Self> {
        let path = path
            .to_str()
            .ok_or(Error::new(E_FAIL, "Only utf-8 paths are supported"))?;

        let handle = unsafe { BeginUpdateResourceW(pass_text(path), allow_overwrite.into()) }?;

        Ok(Self { handle })
    }

    pub fn write<Kind: resource_kind::T>(
        &mut self,
        id: impl Into<ResourceId>,
        data: Kind::Repr<'_>,
    ) -> Result<()> {
        let bytes = Kind::to_bytes(data);

        let lptype = int_resource(Kind::ID);
        let lpname = id.into().lpname;
        let lpdata = Some(bytes.as_ptr() as _);
        let wlanguage = LANG;
        let cb = bytes.len().try_into().map_err(|_| {
            Error::new(
                MEM_E_INVALID_SIZE,
                "Buffer size too big for a resource update",
            )
        })?;

        unsafe { UpdateResourceW(self.handle, lptype, lpname, wlanguage, lpdata, cb) }
    }

    pub fn commit(self) -> Result<()> {
        let this = std::mem::ManuallyDrop::new(self);
        unsafe { EndUpdateResourceW(this.handle, false) }
    }
}

impl Drop for ResourceUpdateTx {
    fn drop(&mut self) {
        let _ = unsafe { EndUpdateResourceW(self.handle, true) };
    }
}
