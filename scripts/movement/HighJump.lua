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

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)

    local ok_g, on_ground = pcall(function() return player:on_ground() end)
    if not ok_g or on_ground then return end

    if vy > 0 and math.abs(vy - 0.42) < 0.01 then
        pcall(function() player:set_velocity(vx, self.settings.velocity, vz) end)
    end
end

anemoia.register(module)
