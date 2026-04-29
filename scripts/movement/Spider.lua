local module = {
    name = "Spider",
    description = "Allows you to climb walls like a spider",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if player:is_collided_horizontally() then -- Wait, we need collided horizontally!
        local vx, vy, vz = table.unpack(player:velocity())
        if vy < 0.15 then
            player:set_velocity(vx, 0.15, vz)
        end
    end
end

anemoia.register(module)
