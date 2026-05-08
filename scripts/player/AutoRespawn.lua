local module = {
    name = "AutoRespawn",
    description = "Automatically respawns when you die",
    category = "Player",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok, dead = pcall(function() return player:is_dead() end)
    if ok and dead then
        pcall(function() player:respawn() end)
    end
end

anemoia.register(module)
