local module = {
    name = "AntiBlind",
    description = "Removes blindness effect",
    category = "Render",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok, has = pcall(function() return player:has_effect("blindness") end)
    if ok and has then
        pcall(function() player:remove_effect("blindness") end)
    end
end

anemoia.register(module)
