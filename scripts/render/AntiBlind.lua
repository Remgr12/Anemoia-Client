local module = {
    name = "AntiBlind",
    description = "Removes blindness effect",
    category = "Render",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if player:has_effect("blindness") then
        player:remove_effect("blindness")
    end
end

anemoia.register(module)
