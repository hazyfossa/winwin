use std::marker::PhantomData;

use windows::{
    Win32::{
        Foundation::{E_POINTER, HMODULE, HRSRC},
        System::LibraryLoader::{
            FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            LoadLibraryExW, LoadResource, LockResource, SizeofResource,
        },
    },
    core::{Error, Free, PCWSTR, Result},
};

use crate::pass_text;

pub const fn int_resource(id: u16) -> PCWSTR {
    PCWSTR::from_raw(id as usize as *const u16)
}

struct ResourceId {
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

pub enum ResourceKind {
    Data,
}

impl ResourceKind {
    // ref: https://learn.microsoft.com/en-us/windows/win32/menurc/resource-types
    fn into_id(self) -> u16 {
        match self {
            Self::Data => 10,
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

pub struct CurrentExe(());

impl CurrentExe {
    pub fn open() -> Self {
        Self(())
    }

    pub fn resources(&self) -> Resources<'static> {
        // Safety: windows won't clean up resources while
        // the current app is alive
        unsafe { Resources::bind_lifetime(None) }
    }
}

#[derive(Clone, Copy)]
pub struct Resources<'a> {
    module_ref: Option<HMODULE>,
    _p: PhantomData<&'a ()>,
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

    #[allow(private_bounds)]
    pub fn open(&self, kind: ResourceKind, id: impl Into<ResourceId>) -> Option<Resource<'a>> {
        let lpname = id.into().lpname;
        let lptype = int_resource(kind.into_id());
        let hres = unsafe { FindResourceW(self.module_ref, lpname, lptype) };

        if hres.is_invalid() {
            return None;
        }

        Some(Resource {
            parent: *self,
            hres,
        })
    }
}

pub struct Resource<'a> {
    parent: Resources<'a>,
    hres: HRSRC,
}

impl<'a> Resource<'a> {
    pub fn len(&self) -> usize {
        let hmodule = self.parent.module_ref;
        unsafe { SizeofResource(hmodule, self.hres) as usize }
    }

    pub fn load_bytes(&self) -> Result<&'a [u8]> {
        let hmodule = self.parent.module_ref;
        let hmem = unsafe { LoadResource(hmodule, self.hres)? };
        let ptr = unsafe { LockResource(hmem) }; // TODO: unlock guard

        if ptr.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }

        let len = self.len();
        if len == 0 {
            return Ok(&[]);
        }

        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast(), len) };

        Ok(bytes)
    }
}
