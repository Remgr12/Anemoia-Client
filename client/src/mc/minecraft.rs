use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};

use super::paths;

pub struct Minecraft {
    pub jni_ref: GlobalRef,
}

impl Minecraft {
    /// Calls `net.minecraft.client.Minecraft.getInstance()` and returns a handle.
    ///
    /// Returns `None` when called before the game has finished initialising its
    /// singleton (e.g. during loading screen).
    pub fn get_instance(env: &mut JNIEnv) -> Result<Option<Self>> {
        let cls = env.find_class(paths::MINECRAFT)?;
        let obj = env
            .call_static_method(
                &cls,
                "getInstance",
                concat!("()Lnet/minecraft/client/Minecraft;"),
                &[],
            )?
            .l()?;

        if obj.is_null() {
            return Ok(None);
        }

        let global = env.new_global_ref(obj)?;
        Ok(Some(Minecraft { jni_ref: global }))
    }
}
