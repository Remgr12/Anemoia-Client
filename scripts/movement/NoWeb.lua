local module = {
    name = "NoWeb",
    description = "Prevents slowing down in webs",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if player:is_in_web() then
        local vx, vy, vz = table.unpack(player:velocity())
        player:set_velocity(vx * 5.0, vy, vz * 5.0)
    end
end

anemoia.register(module)
