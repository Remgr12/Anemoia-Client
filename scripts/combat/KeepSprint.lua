local module = {
    name = "KeepSprint",
    description = "Prevents losing sprint when attacking",
    category = "Combat",
    enabled = false,
}

-- In Minecraft, you lose sprint when you attack.
-- This module simply sets sprinting back to true every tick if moving.

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local is_moving = mc.is_key_down(87) or mc.is_key_down(65) or
                      mc.is_key_down(83) or mc.is_key_down(68)
    if is_moving then
        pcall(function() player:set_sprinting(true) end)
    end
end

anemoia.register(module)
