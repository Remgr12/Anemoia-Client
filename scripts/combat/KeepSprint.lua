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

    local input = player:input()
    if input.up then
        player:set_sprinting(true)
    end
end

anemoia.register(module)
