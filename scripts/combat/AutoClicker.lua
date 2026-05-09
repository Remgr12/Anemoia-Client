local module = {
    name = "AutoClicker",
    description = "Clicks automatically when holding down a mouse button",
    category = "Combat",
    enabled = false,
    settings = {
        mode = "Left", -- Left, Right, Both
        attack_cps = 10,
        use_cps = 20,
        objective = "Any", -- Enemy, Entity, Block, Any
        on_item_use = "Wait", -- Wait, Stop, Ignore
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Left", "Right", "Both" } },
        objective = { type = "enum", options = { "Enemy", "Entity", "Block", "Any" } },
        on_item_use = { type = "enum", options = { "Wait", "Stop", "Ignore" } },
        attack_cps = { min = 1, max = 20 },
        use_cps = { min = 1, max = 20 }
    },
    last_attack = 0,
    last_use = 0
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local mode = self.settings.mode

    -- GLFW_MOUSE_BUTTON_LEFT = 0, GLFW_MOUSE_BUTTON_RIGHT = 1
    -- Anemoia is_key_down uses GLFW codes. 
    -- Mouse buttons are typically 0..7
    
    if (mode == "Left" or mode == "Both") and mc.is_key_down(0) then
        pcall(function() self:handle_attack(player) end)
    end

    if (mode == "Right" or mode == "Both") and mc.is_key_down(1) then
        pcall(function() self:handle_use(player) end)
    end
end

function module:handle_attack(player)
    local now = os.clock() * 1000
    if now - self.last_attack < (1000 / self.settings.attack_cps) then
        return
    end

    local hr = mc.hit_result()
    local should_click = false

    if not hr then
        should_click = (self.settings.objective == "Any")
    else
        local ok_type, type = pcall(function() return hr:type() end)
        if not ok_type then return end
        local obj = self.settings.objective

        if obj == "Any" then
            should_click = true
        elseif obj == "Enemy" and type == "ENTITY" then
            local ok_e, target = pcall(function() return hr:entity() end)
            if ok_e and target and target:alive() and not target:is_local_player() then
                should_click = true
            end
        elseif obj == "Entity" and type == "ENTITY" then
            should_click = true
        elseif obj == "Block" and type == "BLOCK" then
            should_click = true
        end
    end

    if should_click then
        local ok_use, using = pcall(function() return player:is_using_item() end)
        if ok_use and using then
            local action = self.settings.on_item_use
            if action == "Wait" then
                return
            elseif action == "Stop" then
                pcall(function() player:stop_using_item() end)
            end
        end

        pcall(function() mc.click() end)
        self.last_attack = now
    end
end

function module:handle_use(player)
    local now = os.clock() * 1000
    if now - self.last_use < (1000 / self.settings.use_cps) then
        return
    end

    pcall(function() mc.set_right_click_delay(0) end)
    pcall(function() mc.right_click() end)
    self.last_use = now
end

anemoia.register(module)
