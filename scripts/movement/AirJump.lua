local module = {
    name = "AirJump",
    description = "Allows you to jump in mid-air",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local input = player:input()
    if input.jumping and not player:on_ground() then
        local vx, vy, vz = table.unpack(player:velocity())
        if vy < 0 then -- only jump when falling
            player:set_velocity(vx, 0.42, vz)
        end
    end
end

anemoia.register(module)
