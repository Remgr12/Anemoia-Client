local module = {
    name = "Reach",
    description = "Increases your reach distance",
    category = "Player",
    enabled = false,
    settings = {
        distance = 4.5,
        cps      = 10,
    },
    _settings_meta = {
        distance = { min = 3.0, max = 6.0 },
        cps      = { min = 1,   max = 20  },
    },
    _last_attack = 0,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if not mc.is_key_down(0) then return end

    local now = os.clock() * 1000
    if now - self._last_attack < (1000.0 / self.settings.cps) then return end

    local px, py, pz = player:x(), player:y() + 1.62, player:z()
    local range_sq = self.settings.distance * self.settings.distance

    for _, entity in ipairs(mc.entities()) do
        if entity:alive() and not entity:is_local_player() then
            if entity:dist_sq(px, py - 1.62, pz) < range_sq then
                mc.attack(entity)
                self._last_attack = now
                break
            end
        end
    end
end

anemoia.register(module)
