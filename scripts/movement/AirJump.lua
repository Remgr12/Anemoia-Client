local module = {
    name = "AirJump",
    description = "Allows you to jump in mid-air",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if not mc.is_key_down(32) then return end  -- Space = GLFW_KEY_SPACE

    local ok_g, on_ground = pcall(function() return player:on_ground() end)
    if not ok_g or on_ground then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)
    if vy < 0 then
        pcall(function() player:set_velocity(vx, 0.42, vz) end)
    end
end

anemoia.register(module)
