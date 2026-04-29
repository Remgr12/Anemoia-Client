local module = {
    name = "Sprint",
    description = "Automatically sprints while moving",
    category = "Movement",
    enabled = false,
    settings = {
        mode = "Legit", -- Legit, Omnidirectional
        blindness = true,
        hunger = true,
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Legit", "Omnidirectional" } }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local input = player:input()
    
    -- Basic movement check: is the player pushing any movement keys?
    local is_moving = input.up or input.down or input.left or input.right
    if not is_moving then
        return
    end

    -- LiquidBounce checks:
    -- 1. Blindness
    if not self.settings.blindness and player:has_effect("blindness") then
        return
    end

    -- 2. Hunger (food level > 6 is required for sprinting in vanilla)
    if not self.settings.hunger and player:food_level() <= 6 then
        return
    end

    -- 3. Using item
    if player:is_using_item() then
        return
    end

    -- 4. Sneaking
    if input.sneaking then
        return
    end

    local mode = self.settings.mode
    if mode == "Legit" then
        -- In legit mode, we only sprint if moving forward
        if input.up then
            player:set_sprinting(true)
        end
    elseif mode == "Omnidirectional" then
        -- In omni mode, we sprint if moving in any direction
        player:set_sprinting(true)
    end
end

function module:on_disable()
    local player = mc.player()
    if player then
        player:set_sprinting(false)
    end
end

anemoia.register(module)
