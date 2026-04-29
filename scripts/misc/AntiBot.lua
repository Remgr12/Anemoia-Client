local module = {
    name = "AntiBot",
    description = "Prevents attacking bots",
    category = "Misc",
    enabled = false,
    settings = {
        check_name = true,
        check_alive = true,
    }
}

-- This is a simple AntiBot. Real ones are more complex.
function module:is_bot(entity)
    if not entity:alive() and self.settings.check_alive then
        return true
    end
    
    local name = entity:name()
    -- Common bot traits (empty name, non-player type if expecting players)
    if self.settings.check_name and (name == "" or name == nil) then
        return true
    end
    
    return false
end

anemoia.register(module)
