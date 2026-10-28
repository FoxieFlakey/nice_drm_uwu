use std::{collections::HashMap, io, sync::Arc};

use drm::control::{Device, ResourceHandle, property};
use parking_lot::RwLock;

use crate::primary_gpu::PrimaryGpu;

pub trait HasProperties {
    type HandleType: ResourceHandle;

    fn get_primary_gpu(&self) -> Arc<PrimaryGpu>;
    fn get_cache(&self) -> &RwLock<HashMap<String, (property::Handle, property::Info)>>;
    fn get_handle(&self) -> Self::HandleType;

    #[expect(unused)]
    fn set_property<S>(&self, name: &S, value: u64) -> Result<(), io::Error>
    where
        S: AsRef<str> + ?Sized,
    {
        let Some((handle, info)) = self.get_property_handle(name)? else {
            return Err(io::ErrorKind::NotFound.into());
        };

        assert!(info.mutable(), "Attempt to modify immutable property");

        let gpu = self.get_primary_gpu();
        gpu.fd.set_property(self.get_handle(), handle, value)?;
        Ok(())
    }

    fn get_property_handle<S>(
        &self,
        name: &S,
    ) -> Result<Option<(property::Handle, property::Info)>, io::Error>
    where
        S: AsRef<str> + ?Sized,
    {
        let cache = self.get_cache();
        let cache_guard = cache.read();
        if let Some(x) = cache_guard.get(name.as_ref()) {
            return Ok(Some(x.clone()));
        }
        drop(cache_guard);

        // Missed the cache find it
        let mut cache_guard = cache.write();
        if let Some(x) = cache_guard.get(name.as_ref()) {
            // Someone cached it between unlocking read and locking as write
            return Ok(Some(x.clone()));
        }

        let gpu = self.get_primary_gpu();
        let mut ret = None;
        for (handle, _) in gpu.fd.get_properties(self.get_handle())? {
            let prop = gpu.fd.get_property(handle)?;
            let Ok(prop_name) = prop.name().to_str() else {
                // If OsStr is not valid Rust string then there
                // no reason to cache it because nothing will ever
                // match
                continue;
            };

            if prop_name == name.as_ref() {
                ret = Some((handle, prop.clone()));
            }

            assert!(
                cache_guard
                    .insert(prop_name.to_string(), (handle, prop))
                    .is_none(),
                "There shouldn't be duplicate property!"
            );
        }

        Ok(ret)
    }
}
