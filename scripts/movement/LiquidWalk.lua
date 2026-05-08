local module = {
    name = "LiquidWalk",
    description = "Allows you to walk on liquids",
    category = "Movement",
    enabled = false,
    settings = {
        velocity = 0.1,
    },
    _settings_meta = {
        velocity = { min = 0.01, max = 0.5 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_liq, in_liquid = pcall(function()
        return player:is_in_water() or player:is_in_lava()
    end)
    if not ok_liq or not in_liquid then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, _, vz = table.unpack(vel)
    pcall(function() player:set_velocity(vx, self.settings.velocity, vz) end)
end

anemoia.register(module)
