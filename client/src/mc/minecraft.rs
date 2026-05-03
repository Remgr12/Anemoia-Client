use anyhow::Result;
use jni::{objects::{GlobalRef, JValue}, JNIEnv};

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
        let cls = crate::jvm::Jvm::find_class(env, paths::MINECRAFT)?;
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

    pub fn left_click_mouse(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "leftClickMouse", "()V", &[])?;
        Ok(())
    }

    pub fn right_click_mouse(&self, env: &mut JNIEnv) -> Result<()> {
        env.call_method(self.jni_ref.as_obj(), "rightClickMouse", "()V", &[])?;
        Ok(())
    }

    pub fn use_item(&self, env: &mut JNIEnv, hand: &str) -> Result<()> {
        let gamemode = env.get_field(self.jni_ref.as_obj(), "gameMode", "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;")?.l()?;
        let player = env.get_field(self.jni_ref.as_obj(), "player", "Lnet/minecraft/client/player/LocalPlayer;")?.l()?;
        
        let hand_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/InteractionHand")?;
        let hand_obj = env.get_static_field(hand_cls, hand, "Lnet/minecraft/world/InteractionHand;")?.l()?;

        env.call_method(&gamemode, "useItem", "(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;", &[
            jni::objects::JValue::Object(&player),
            jni::objects::JValue::Object(&hand_obj),
        ])?;
        Ok(())
    }

    pub fn get_hit_result(&self, env: &mut JNIEnv) -> Result<Option<super::world::HitResult>> {
        let obj = env.get_field(self.jni_ref.as_obj(), "hitResult", "Lnet/minecraft/world/phys/HitResult;")?.l()?;
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(super::world::HitResult { jni_ref: env.new_global_ref(obj)? }))
    }

    pub fn get_current_screen(&self, env: &mut JNIEnv) -> Result<Option<GlobalRef>> {
        let obj = env.get_field(self.jni_ref.as_obj(), "screen", "Lnet/minecraft/client/gui/screens/Screen;")?.l()?;
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(env.new_global_ref(obj)?))
    }

    pub fn get_right_click_delay_timer(&self, env: &mut JNIEnv) -> Result<i32> {
        Ok(env.get_field(self.jni_ref.as_obj(), "rightClickDelayTimer", "I")?.i()?)
    }

    pub fn set_right_click_delay_timer(&self, env: &mut JNIEnv, value: i32) -> Result<()> {
        env.set_field(self.jni_ref.as_obj(), "rightClickDelayTimer", "I", jni::objects::JValue::Int(value))?;
        Ok(())
    }

    pub fn set_gamma(&self, env: &mut JNIEnv, value: f64) -> Result<()> {
        let options = env.get_field(self.jni_ref.as_obj(), "options", "Lnet/minecraft/client/Options;")?.l()?;
        let gamma_option = env.get_field(&options, "gamma", "Lnet/minecraft/client/OptionInstance;")?.l()?;
        
        let double_val = env.new_object("java/lang/Double", "(D)V", &[jni::objects::JValue::Double(value)])?;
        env.call_method(&gamma_option, "set", "(Ljava/lang/Object;)V", &[
            jni::objects::JValue::Object(&double_val)
        ])?;
        Ok(())
    }

    /// Places a block by directly constructing a `BlockHitResult` for the given face and calling
    /// `gameMode.useItemOn()`. Bypasses `mc.hitResult` entirely so no rotation is required.
    ///
    /// `face` must be one of "DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST".
    /// Returns true if the server-bound interaction was consumed (block placed).
    pub fn place_block_on_face(&self, env: &mut JNIEnv, x: i32, y: i32, z: i32, face: &str) -> Result<bool> {
        env.push_local_frame(24)?;

        let result: Result<bool> = (|| {
            let gamemode = env
                .get_field(self.jni_ref.as_obj(), "gameMode",
                    "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;")?.l()?;
            let player = env
                .get_field(self.jni_ref.as_obj(), "player",
                    "Lnet/minecraft/client/player/LocalPlayer;")?.l()?;
            anyhow::ensure!(!gamemode.is_null(), "gameMode is null");
            anyhow::ensure!(!player.is_null(), "player is null");

            let bp_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/core/BlockPos")?;
            let block_pos = env.new_object(&bp_cls, "(III)V", &[
                JValue::Int(x), JValue::Int(y), JValue::Int(z),
            ])?;

            let dir_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/core/Direction")?;
            let direction = env.get_static_field(
                &dir_cls, face, "Lnet/minecraft/core/Direction;")?.l()?;

            let (hx, hy, hz) = match face {
                "UP"    => (x as f64 + 0.5, y as f64 + 1.0, z as f64 + 0.5),
                "DOWN"  => (x as f64 + 0.5, y as f64,       z as f64 + 0.5),
                "NORTH" => (x as f64 + 0.5, y as f64 + 0.5, z as f64),
                "SOUTH" => (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 1.0),
                "WEST"  => (x as f64,       y as f64 + 0.5, z as f64 + 0.5),
                "EAST"  => (x as f64 + 1.0, y as f64 + 0.5, z as f64 + 0.5),
                other   => anyhow::bail!("unknown face: {}", other),
            };

            let vec3_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/phys/Vec3")?;
            let hit_vec = env.new_object(&vec3_cls, "(DDD)V", &[
                JValue::Double(hx), JValue::Double(hy), JValue::Double(hz),
            ])?;

            let bhr_cls = crate::jvm::Jvm::find_class(
                env, "net/minecraft/world/phys/BlockHitResult")?;
            let hit_result = env.new_object(
                &bhr_cls,
                "(Lnet/minecraft/world/phys/Vec3;Lnet/minecraft/core/Direction;\
                  Lnet/minecraft/core/BlockPos;Z)V",
                &[
                    JValue::Object(&hit_vec),
                    JValue::Object(&direction),
                    JValue::Object(&block_pos),
                    JValue::Bool(0),
                ],
            )?;

            let hand_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/InteractionHand")?;
            let main_hand = env.get_static_field(
                &hand_cls, "MAIN_HAND", "Lnet/minecraft/world/InteractionHand;")?.l()?;

            let result_obj = env.call_method(
                &gamemode,
                "useItemOn",
                "(Lnet/minecraft/world/entity/player/Player;\
                  Lnet/minecraft/world/InteractionHand;\
                  Lnet/minecraft/world/phys/BlockHitResult;)\
                  Lnet/minecraft/world/InteractionResult;",
                &[
                    JValue::Object(&player),
                    JValue::Object(&main_hand),
                    JValue::Object(&hit_result),
                ],
            )?.l()?;

            if result_obj.is_null() {
                return Ok(false);
            }
            let consumed = env.call_method(&result_obj, "consumesAction", "()Z", &[])?.z()?;
            Ok(consumed)
        })();

        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let _ = unsafe { env.pop_local_frame(&jni::objects::JObject::null()) };
        result
    }

    pub fn inventory_click(&self, env: &mut JNIEnv, container_id: i32, slot: i32, _button: i32, click_type: &str) -> Result<()> {
        let gamemode = env.get_field(self.jni_ref.as_obj(), "gameMode", "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;")?.l()?;
        let click_type_cls = crate::jvm::Jvm::find_class(env, "net/minecraft/world/inventory/ClickType")?;
        let click_type_obj = env.get_static_field(click_type_cls, click_type, "Lnet/minecraft/world/inventory/ClickType;")?.l()?;
        
        let player = env.get_field(self.jni_ref.as_obj(), "player", "Lnet/minecraft/client/player/LocalPlayer;")?.l()?;

        env.call_method(&gamemode, "handleInventoryMouseClick", "(IILnet/minecraft/world/inventory/ClickType;Lnet/minecraft/world/entity/player/Player;)V", &[
            jni::objects::JValue::Int(container_id),
            jni::objects::JValue::Int(slot),
            jni::objects::JValue::Object(&click_type_obj),
            jni::objects::JValue::Object(&player),
        ])?;
        Ok(())
    }
}
