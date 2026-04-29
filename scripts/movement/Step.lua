local module = {
    name = "Step",
    description = "Allows you to step up full blocks",
    category = "Movement",
    enabled = false,
    settings = {
        height = 1.0,
    },
    _settings_meta = {
        height = { min = 0.6, max = 2.5 }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    player:set_step_height(self.settings.height)
end

function module:on_disable()
    local player = mc.player()
    if player then
        player:set_step_height(0.6)
    end
end

anemoia.register(module)
