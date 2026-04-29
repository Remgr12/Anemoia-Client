local ui = {}

function ui.draw(painter, module)
    local x, y = 10, 10
    
    painter:text(x, y, "Zulip Bridge Chat", 255, 255, 255, 255, 16)
    
    local msg_y = y + 30
    local limit = 15
    local start = math.max(1, #module.messages - limit + 1)
    for i = start, #module.messages do
        local m = module.messages[i]
        local line = "[" .. m.time .. "] " .. m.sender .. ": " .. m.content
        painter:text(x, msg_y, line, 220, 220, 220, 255, 14)
        msg_y = msg_y + 18
    end

    painter:text(x, msg_y + 20, "Status: " .. (module.queue_id and "Connected" or "Disconnected"), 100, 255, 100, 255, 12)
end

return ui
