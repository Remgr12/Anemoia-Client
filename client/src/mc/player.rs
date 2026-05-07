use anyhow::Result;
use jni::{objects::GlobalRef, JNIEnv};

use super::{minecraft::Minecraft, packet::Connection, item::ItemStack};

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

    pub fn on_ground(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "onGround", "()Z", &[])?
            .z()?)
    }

    pub fn set_on_ground(&self, env: &mut JNIEnv, on_ground: bool) -> Result<()> {
        env.set_field(
            self.jni_ref.as_obj(),
            "onGround",
            "Z",
            jni::objects::JValue::Bool(on_ground as jni::sys::jboolean),
        )?;
        Ok(())
    }

    pub fn is_sprinting(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env
            .call_method(self.jni_ref.as_obj(), "isSprinting", "()Z", &[])?
            .z()?)
    }

    pub fn set_sprinting(&self, env: &mut JNIEnv, sprinting: bool) -> Result<()> {
        env.call_method(
            self.jni_ref.as_obj(),
            "setSprinting",
            "(Z)V",
            &[jni::objects::JValue::Bool(sprinting as jni::sys::jboolean)],
        )?;
        Ok(())
    }

    pub fn get_fall_distance(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "fallDistance", "F")?.f()?)
    }

    pub fn get_hurt_time(&self, env: &mut JNIEnv) -> Result<i32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "hurtTime", "I")?.i()?)
    }

    pub fn get_input(&self, env: &mut JNIEnv) -> Result<PlayerInput> {
        let input_obj = env
            .get_field(self.jni_ref.as_obj(), "input", "Lnet/minecraft/client/player/Input;")?
            .l()?;
        
        let left = env.get_field(&input_obj, "left", "Z")?.z()?;
        let right = env.get_field(&input_obj, "right", "Z")?.z()?;
        let up = env.get_field(&input_obj, "up", "Z")?.z()?;
        let down = env.get_field(&input_obj, "down", "Z")?.z()?;
        let jumping = env.get_field(&input_obj, "jumping", "Z")?.z()?;
        let shift_key_down = env.get_field(&input_obj, "shiftKeyDown", "Z")?.z()?;

        Ok(PlayerInput {
            left,
            right,
            up,
            down,
            jumping,
            shift_key_down,
        })
    }

    pub fn get_inventory(&self, env: &mut JNIEnv) -> Result<PlayerInventory> {
        let inv_obj = env
            .get_field(
                self.jni_ref.as_obj(),
                "inventory",
                "Lnet/minecraft/world/entity/player/Inventory;",
            )?
            .l()?;
        Ok(PlayerInventory {
            jni_ref: env.new_global_ref(inv_obj)?,
        })
    }

    pub fn get_connection(&self, env: &mut JNIEnv) -> Result<Connection> {
        let listener_obj = env.get_field(
            self.jni_ref.as_obj(),
            "connection",
            "Lnet/minecraft/client/multiplayer/ClientPacketListener;"
        )?.l()?;
        
        let connection_obj = env.get_field(
            &listener_obj,
            "connection",
            "Lnet/minecraft/network/Connection;"
        )?.l()?;

        Ok(Connection {
            jni_ref: env.new_global_ref(connection_obj)?
        })
    }

    pub fn is_using_item(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.call_method(self.jni_ref.as_obj(), "isUsingItem", "()Z", &[])?.z()?)
    }

    pub fn stop_using_item(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "stopUsingItem", "()V", &[])?;
        Ok(())
    }

    pub fn get_food_level(&self, env: &mut JNIEnv) -> Result<i32> {
        let food_data = env.call_method(self.jni_ref.as_obj(), "getFoodData", "()Lnet/minecraft/world/food/FoodData;", &[])?.l()?;
        Ok(env.call_method(&food_data, "getFoodLevel", "()I", &[])?.i()?)
    }

    pub fn get_main_hand_item(&self, env: &mut JNIEnv) -> Result<ItemStack> {
        let obj = env.call_method(self.jni_ref.as_obj(), "getMainHandItem", "()Lnet/minecraft/world/item/ItemStack;", &[])?.l()?;
        Ok(ItemStack { jni_ref: env.new_global_ref(obj)? })
    }

    pub fn get_off_hand_item(&self, env: &mut JNIEnv) -> Result<ItemStack> {
        let obj = env.call_method(self.jni_ref.as_obj(), "getOffhandItem", "()Lnet/minecraft/world/item/ItemStack;", &[])?.l()?;
        Ok(ItemStack { jni_ref: env.new_global_ref(obj)? })
    }

    pub fn has_effect(&self, env: &mut JNIEnv, effect_obj: jni::objects::JObject) -> Result<bool> {
        Ok(env.call_method(
            self.jni_ref.as_obj(),
            "hasEffect",
            "(Lnet/minecraft/world/effect/MobEffect;)Z",
            &[jni::objects::JValue::Object(&effect_obj)],
        )?.z()?)
    }

    pub fn jump(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "jumpFromGround", "()V", &[])?;
        Ok(())
    }

    pub fn get_step_height(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "maxUpStep", "F")?.f()?)
    }

    pub fn set_step_height(&self, env: &mut JNIEnv, height: f32) -> Result<()> {
        env.set_field(self.jni_ref.as_obj(), "maxUpStep", "F", jni::objects::JValue::Float(height))?;
        Ok(())
    }

    pub fn is_collided_horizontally(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.get_field(self.jni_ref.as_obj(), "horizontalCollision", "Z")?.z()?)
    }

    pub fn is_in_water(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.call_method(self.jni_ref.as_obj(), "isInWater", "()Z", &[])?.z()?)
    }

    pub fn is_in_lava(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.call_method(self.jni_ref.as_obj(), "isInLava", "()Z", &[])?.z()?)
    }

    pub fn is_dead(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.get_field(self.jni_ref.as_obj(), "dead", "Z")?.z()?)
    }

    pub fn respawn(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "respawn", "()V", &[])?;
        Ok(())
    }

    pub fn is_in_web(&self, env: &mut JNIEnv) -> Result<bool> {
        match env.get_field(self.jni_ref.as_obj(), "inWebOrSweetBerryBush", "Z") {
            Ok(v) => Ok(v.z().unwrap_or(false)),
            Err(_) => { let _ = env.exception_clear(); Ok(false) }
        }
    }

    pub fn get_health(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getHealth", "()F", &[])?.f()?)
    }

    pub fn get_max_health(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getMaxHealth", "()F", &[])?.f()?)
    }

    pub fn get_absorption_amount(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getAbsorptionAmount", "()F", &[])?.f()?)
    }

    pub fn swing_arm(&self, env: &mut JNIEnv, hand: &str) -> Result<()> {
        let hand_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/InteractionHand")?;
        let hand_field = if hand.eq_ignore_ascii_case("OFF_HAND") { "OFF_HAND" } else { "MAIN_HAND" };
        let hand_obj = env.get_static_field(hand_cls, hand_field, "Lnet/minecraft/world/InteractionHand;")?.l()?;
        env.call_method(
            self.jni_ref.as_obj(),
            "swing",
            "(Lnet/minecraft/world/InteractionHand;)V",
            &[jni::objects::JValue::Object(&hand_obj)],
        )?;
        Ok(())
    }

    pub fn send_chat(&self, env: &mut JNIEnv, message: &str) -> Result<()> {
        let listener_obj = env.get_field(
            self.jni_ref.as_obj(),
            "connection",
            "Lnet/minecraft/client/multiplayer/ClientPacketListener;",
        )?.l()?;
        let msg_jstr = env.new_string(message)?;
        env.call_method(
            &listener_obj,
            "sendChat",
            "(Ljava/lang/String;)V",
            &[jni::objects::JValue::Object(&msg_jstr.into())],
        )?;
        Ok(())
    }

    pub fn remove_effect(&self, env: &mut JNIEnv, effect_obj: jni::objects::JObject) -> Result<()> {
        env.call_method(
            self.jni_ref.as_obj(),
            "removeEffect",
            "(Lnet/minecraft/world/effect/MobEffect;)Z",
            &[jni::objects::JValue::Object(&effect_obj)],
        )?;
        Ok(())
    }

    pub fn get_container_id(&self, env: &mut JNIEnv) -> Result<i32> {
        let container = env.get_field(self.jni_ref.as_obj(), "containerMenu", "Lnet/minecraft/world/inventory/AbstractContainerMenu;")?.l()?;
        Ok(env.get_field(&container, "containerId", "I")?.i()?)
    }

    pub fn display_message(&self, env: &mut JNIEnv, message: &str) -> Result<()> {
        let component_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/network/chat/Component")?;
        let msg_jstr = env.new_string(message)?;
        let component = env.call_static_method(
            component_cls,
            "literal",
            "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            &[jni::objects::JValue::Object(&msg_jstr.into())],
        )?.l()?;

        env.call_method(
            self.jni_ref.as_obj(),
            "displayClientMessage",
            "(Lnet/minecraft/network/chat/Component;Z)V",
            &[
                jni::objects::JValue::Object(&component),
                jni::objects::JValue::Bool(0),
            ],
        )?;
        Ok(())
    }
}

pub struct PlayerInventory {
    pub jni_ref: GlobalRef,
}

impl PlayerInventory {
    pub fn get_selected_slot(&self, env: &mut JNIEnv) -> Result<i32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "selected", "I")?.i()?)
    }

    pub fn set_selected_slot(&self, env: &mut JNIEnv, slot: i32) -> Result<()> {
        env.set_field(
            self.jni_ref.as_obj(),
            "selected",
            "I",
            jni::objects::JValue::Int(slot),
        )?;
        Ok(())
    }
}

pub struct PlayerInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jumping: bool,
    pub shift_key_down: bool,
}
