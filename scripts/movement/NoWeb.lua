local module = {
    name = "NoWeb",
    description = "Prevents slowing down in webs",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_web, in_web = pcall(function() return player:is_in_web() end)
    if not ok_web or not in_web then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)
    pcall(function() player:set_velocity(vx * 5.0, vy, vz * 5.0) end)
end

anemoia.register(module)
