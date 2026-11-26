use windows::{
    Win32::{
        Foundation::{E_POINTER, HMODULE},
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

pub struct Exe {
    module: Option<HMODULE>,
}

impl Exe {
    pub fn current() -> Self {
        Self { module: None }
    }

    pub fn open(path: &str) -> Result<Self> {
        let module = unsafe {
            LoadLibraryExW(
                pass_text(path),
                None,
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )?
        };
        Ok(Self {
            module: Some(module),
        })
    }

    #[allow(private_bounds)]
    pub fn open_resource(
        &self,
        kind: ResourceKind,
        id: impl Into<ResourceId>,
    ) -> Result<Option<&[u8]>> {
        let lpname = id.into().lpname;
        let lptype = int_resource(kind.into_id());
        let hres = unsafe { FindResourceW(self.module, lpname, lptype) };

        if hres.is_invalid() {
            return Ok(None);
        }

        let len = unsafe { SizeofResource(self.module, hres) } as usize;
        let hmem = unsafe { LoadResource(self.module, hres)? };
        let ptr = unsafe { LockResource(hmem) };

        if ptr.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }

        if len == 0 {
            return Ok(Some(&[]));
        }

        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast(), len) };

        Ok(Some(bytes))
    }
}

impl Drop for Exe {
    fn drop(&mut self) {
        if let Some(mut remote) = self.module {
            unsafe { remote.free() };
        }
    }
}
