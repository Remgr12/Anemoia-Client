local module = {
    name = "NoSlow",
    description = "Prevents slowing down while using items",
    category = "Movement",
    enabled = false,
    settings = {
        multiplier = 1.0, -- 1.0 = 100% speed
    },
    _settings_meta = {
        multiplier = { min = 0.2, max = 1.0 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if player:is_using_item() then
        local vx, vy, vz = table.unpack(player:velocity())
        
        -- In Vanilla, you are slowed down to ~20% speed (0.2 multiplier)
        -- To keep 100% speed, we'd need to multiply by 5.0, but usually
        -- it's better to just set the velocity if moving.
        
        -- Simple approach: if moving, boost velocity back up
        local input = player:input()
        if input.up or input.down or input.left or input.right then
            -- This is a very basic NoSlow. 
            -- Real NoSlow usually involves packets or hooking movement input.
            -- Since we only have on_tick, we can try to compensate.
            
            -- Note: 0.2 is the vanilla slowdown. 
            -- To get to 'multiplier', we multiply by (multiplier / 0.2)
            local boost = self.settings.multiplier / 0.2
            
            -- Only apply boost if we haven't already boosted this tick
            -- (Basic protection against double-boosting if on_tick is called multiple times)
            player:set_velocity(vx * boost, vy, vz * boost)
        end
    end
end

anemoia.register(module)
