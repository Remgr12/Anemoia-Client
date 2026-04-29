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

    if player:is_in_water() or player:is_in_lava() then
        local vx, vy, vz = table.unpack(player:velocity())
        player:set_velocity(vx, self.settings.velocity, vz)
    end
end

anemoia.register(module)
