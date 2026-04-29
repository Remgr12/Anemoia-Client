local module = {
    name = "AutoRespawn",
    description = "Automatically respawns when you die",
    category = "Player",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if player:is_dead() then
        player:respawn()
    end
end

anemoia.register(module)
