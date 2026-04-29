use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};

pub struct ItemStack {
    pub jni_ref: GlobalRef,
}

impl ItemStack {
    pub fn type_id(&self, env: &mut JNIEnv) -> Result<String> {
        let item = env.call_method(self.jni_ref.as_obj(), "getItem", "()Lnet/minecraft/world/item/Item;", &[])?.l()?;
        let description_id = env.call_method(&item, "getDescriptionId", "()Ljava/lang/String;", &[])?.l()?;
        let s: String = env.get_string((&description_id).into())?.into();
        Ok(s)
    }

    pub fn is_empty(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.call_method(self.jni_ref.as_obj(), "isEmpty", "()Z", &[])?.z()?)
    }
}
