local module = {
    name = "HighJump",
    description = "Increases your jump height",
    category = "Movement",
    enabled = false,
    settings = {
        velocity = 0.7,
    },
    _settings_meta = {
        velocity = { min = 0.42, max = 2.0 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local vx, vy, vz = table.unpack(player:velocity())
    if vy > 0 and not player:on_ground() then
        -- If we just jumped (vy is roughly 0.42 in vanilla)
        if math.abs(vy - 0.42) < 0.01 then
            player:set_velocity(vx, self.settings.velocity, vz)
        end
    end
end

anemoia.register(module)
