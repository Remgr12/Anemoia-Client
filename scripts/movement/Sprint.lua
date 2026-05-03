local module = {
    name        = "Sprint",
    description = "Automatically sprints while moving",
    category    = "Movement",
    enabled     = false,
    settings = {
        mode      = "Legit",
        blindness = true,
        hunger    = true,
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Legit", "Omnidirectional" } }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_input, input = pcall(function() return player:input() end)
    if not ok_input or not input then return end

    local is_moving = input.up or input.down or input.left or input.right
    if not is_moving then return end
    if input.sneaking then return end

    local ok_use, using_item = pcall(function() return player:is_using_item() end)
    if ok_use and using_item then return end

    if not self.settings.blindness then
        local ok_eff, has_blind = pcall(function()
            return player:has_effect("blindness")
        end)
        if ok_eff and has_blind then return end
    end

    if not self.settings.hunger then
        local ok_food, food = pcall(function() return player:food_level() end)
        if ok_food and food <= 6 then return end
    end

    local mode = self.settings.mode
    if mode == "Legit" then
        if input.up then
            pcall(function() player:set_sprinting(true) end)
        end
    elseif mode == "Omnidirectional" then
        pcall(function() player:set_sprinting(true) end)
    end
end

function module:on_disable()
    local player = mc.player()
    if not player then return end
    pcall(function() player:set_sprinting(false) end)
end

anemoia.register(module)
