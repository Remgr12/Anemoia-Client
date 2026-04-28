pub mod minecraft;
pub mod player;
pub mod window;
pub mod world;

// Raw JNI class/method paths for Minecraft Java Edition 26.2 (unobfuscated).
// No mapping layer needed — class names match the shipped bytecode directly.
pub mod paths {
    pub const MINECRAFT: &str = "net/minecraft/client/Minecraft";
    pub const LOCAL_PLAYER: &str = "net/minecraft/client/player/LocalPlayer";
    pub const ENTITY: &str = "net/minecraft/world/entity/Entity";
    pub const LEVEL: &str = "net/minecraft/world/level/Level";
    pub const CLIENT_LEVEL: &str = "net/minecraft/client/multiplayer/ClientLevel";
}
