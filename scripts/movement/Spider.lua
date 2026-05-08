local module = {
    name = "Spider",
    description = "Allows you to climb walls like a spider",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_col, collided = pcall(function() return player:is_collided_horizontally() end)
    if not ok_col or not collided then return end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return end
    local vx, vy, vz = table.unpack(vel)
    if vy < 0.15 then
        pcall(function() player:set_velocity(vx, 0.15, vz) end)
    end
end

anemoia.register(module)
