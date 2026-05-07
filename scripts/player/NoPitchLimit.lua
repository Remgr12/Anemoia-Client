local module = {
    name = "NoPitchLimit",
    description = "Overrides pitch beyond ±90° (packet-level; pair with Aimbot for effect)",
    category = "Player",
    enabled = false,
    settings = {
        override = false,
        pitch    = 90,
    },
    _settings_meta = {
        pitch = { min = -180, max = 180 },
    }
}

-- MC clamps xRot to ±90 during input processing which runs before our tick.
-- set_pitch() fires AFTER input processing but BEFORE the outgoing movement
-- packet, so the server receives the overridden value even though the client
-- view is still clamped. Enable 'override' and set a 'pitch' to use.
function module:on_tick()
    if not self.settings.override then return end
    local player = mc.player()
    if not player then return end
    player:set_pitch(self.settings.pitch)
end

anemoia.register(module)
