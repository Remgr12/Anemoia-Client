use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};
use std::sync::Arc;

use super::minecraft::Minecraft;

/// Wraps a JNI global reference to the local player entity.
///
/// `Arc` + `GlobalRef` are both `Send + Sync`, so `LuaPlayer` (which wraps
/// `Arc<LocalPlayer>`) can be stored as mlua `UserData`.
pub struct LocalPlayer {
    pub jni_ref: GlobalRef,
}

impl LocalPlayer {
    /// Reads `Minecraft.player` and returns a handle, or `None` if not in world.
    pub fn from_minecraft(mc: &Minecraft, env: &mut JNIEnv) -> Result<Option<Self>> {
        let obj = env
            .get_field(
                mc.jni_ref.as_obj(),
                "player",
                "Lnet/minecraft/client/player/LocalPlayer;",
            )?
            .l()?;

        if obj.is_null() {
            return Ok(None);
        }

        Ok(Some(LocalPlayer {
            jni_ref: env.new_global_ref(obj)?,
        }))
    }

    pub fn get_x(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "getX", "()D", &[])?
            .d()?)
    }

    pub fn get_y(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "getY", "()D", &[])?
            .d()?)
    }

    pub fn get_z(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "getZ", "()D", &[])?
            .d()?)
    }

    /// Y rotation (yaw) in degrees.
    pub fn get_yaw(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "getYRot", "()F", &[])?
            .f()?)
    }

    /// X rotation (pitch) in degrees.
    pub fn get_pitch(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "getXRot", "()F", &[])?
            .f()?)
    }

    pub fn set_yaw(&self, env: &mut JNIEnv, yaw: f32) -> Result<()> {
        env.call_method(
            self.jni_ref.as_obj(),
            "setYRot",
            "(F)V",
            &[jni::objects::JValue::Float(yaw)],
        )?;
        Ok(())
    }

    pub fn set_pitch(&self, env: &mut JNIEnv, pitch: f32) -> Result<()> {
        env.call_method(
            self.jni_ref.as_obj(),
            "setXRot",
            "(F)V",
            &[jni::objects::JValue::Float(pitch)],
        )?;
        Ok(())
    }

    /// Returns the delta movement vector as `(dx, dy, dz)`.
    pub fn get_delta_movement(&self, env: &mut JNIEnv) -> Result<(f64, f64, f64)> {
        // getDeltaMovement() → Vec3
        let vec3 = env
            .call_method(
                self.jni_ref.as_obj(),
                "getDeltaMovement",
                "()Lnet/minecraft/world/phys/Vec3;",
                &[],
            )?
            .l()?;

        let x = env.get_field(&vec3, "x", "D")?.d()?;
        let y = env.get_field(&vec3, "y", "D")?.d()?;
        let z = env.get_field(&vec3, "z", "D")?.d()?;
        Ok((x, y, z))
    }

    /// Overwrites the delta movement vector.
    pub fn set_delta_movement(&self, env: &mut JNIEnv, dx: f64, dy: f64, dz: f64) -> Result<()> {
        env.call_method(
            self.jni_ref.as_obj(),
            "setDeltaMovement",
            "(DDD)V",
            &[
                jni::objects::JValue::Double(dx),
                jni::objects::JValue::Double(dy),
                jni::objects::JValue::Double(dz),
            ],
        )?;
        Ok(())
    }
}

/// mlua-compatible wrapper. The `Arc` lets multiple Lua closures share the ref.
pub struct LuaPlayer(pub Arc<LocalPlayer>);
