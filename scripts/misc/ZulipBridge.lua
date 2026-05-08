local module = {
    name = "ZulipBridge",
    description = "Bridges Zulip chat into Minecraft. Use .help in chat for commands.",
    category = "Misc",
    enabled = false,
    hidden = true,
    settings = {
        poll_rate  = 2.0,
        bridge_out = true,
        bridge_in  = true,
    },
    last_msgs     = {},
    mirror_stream = "",
    mirror_topic  = "",
    _last_poll_rate = nil,
}

function module:on_enable()
    self.last_msgs = {}
    self._last_poll_rate = self.settings.poll_rate
    anemoia.zulip_config({
        enabled   = true,
        poll_rate = self.settings.poll_rate,
    })
    anemoia.zulip_clear()
end

function module:on_disable()
    anemoia.zulip_config({ enabled = false })
    self._last_poll_rate = nil
end

function module:on_tick()
    if not self.enabled then return end

    -- Only write to disk when poll_rate actually changes (was: every tick = 60 disk writes/sec).
    if self._last_poll_rate ~= self.settings.poll_rate then
        self._last_poll_rate = self.settings.poll_rate
        anemoia.zulip_config({ enabled = true, poll_rate = self.settings.poll_rate })
    end

    if not self.settings.bridge_in then return end

    local messages = anemoia.zulip_get_messages()
    if #messages == 0 then return end

    local player = mc.player()
    if not player then return end

    for _, msg in ipairs(messages) do
        if self.mirror_stream ~= "" and msg.stream ~= self.mirror_stream then goto continue end
        if self.mirror_topic  ~= "" and msg.topic  ~= self.mirror_topic  then goto continue end

        local id = msg.time .. msg.sender .. msg.content
        local found = false
        for _, cid in ipairs(self.last_msgs) do
            if cid == id then found = true; break end
        end
        if not found then
            player:display_message("§d[Zulip] §f" .. msg.sender .. "§7: §f" .. msg.content)
            table.insert(self.last_msgs, id)
            if #self.last_msgs > 100 then table.remove(self.last_msgs, 1) end
        end
        ::continue::
    end
end

local function cmd_help(player)
    player:display_message("§e§lZulip commands§r §7(.prefix)")
    player:display_message("§e.send <msg>              §7- Send to current target")
    player:display_message("§e.stream <stream> <topic> §7- Set outgoing target")
    player:display_message("§e.status                  §7- Show bridge status")
    player:display_message("§e.enable / .disable       §7- Toggle bridge")
    player:display_message("§e.mirror <stream> <topic> §7- Set MC chat mirror source")
    player:display_message("§e.help                    §7- This list")
end

local function handle_dot_cmd(raw, player)
    local parts = {}
    for p in raw:gmatch("%S+") do table.insert(parts, p) end
    local sub = parts[1] or "help"

    if sub == "help" then
        cmd_help(player)
        return true

    elseif sub == "send" then
        if #parts < 2 then
            player:display_message("§cUsage: .send <message>")
            return true
        end
        anemoia.zulip_send(table.concat(parts, " ", 2))
        return true

    elseif sub == "stream" then
        if #parts < 3 then
            player:display_message("§cUsage: .stream <stream> <topic>")
            return true
        end
        anemoia.zulip_config({ stream = parts[2], topic = table.concat(parts, " ", 3) })
        player:display_message("§aTarget: §f" .. parts[2] .. " › " .. table.concat(parts, " ", 3))
        return true

    elseif sub == "mirror" then
        if #parts < 3 then
            player:display_message("§cUsage: .mirror <stream> <topic>")
            return true
        end
        module.mirror_stream = parts[2]
        module.mirror_topic  = table.concat(parts, " ", 3)
        player:display_message("§aMirror: §f" .. module.mirror_stream .. " › " .. module.mirror_topic)
        return true

    elseif sub == "status" then
        local en = module.enabled and "§aenabled" or "§cdisabled"
        player:display_message("§eBridge: " .. en)
        if module.mirror_stream ~= "" then
            player:display_message("§eMirror: §f" .. module.mirror_stream .. " › " .. module.mirror_topic)
        else
            player:display_message("§eMirror: §7all messages")
        end
        return true

    elseif sub == "enable" then
        module.enabled = true
        player:display_message("§aZulip bridge enabled")
        return true

    elseif sub == "disable" then
        module.enabled = false
        anemoia.zulip_config({ enabled = false })
        player:display_message("§cZulip bridge disabled")
        return true
    end

    return false
end

anemoia.on_packet_send(function(packet)
    local ptype = packet:type_name()
    if not ptype:find("ServerboundChatPacket") then return false end

    local fields = packet:fields()
    local msg = fields.message
    if not msg or msg == "" then return false end

    if msg:sub(1, 1) == "." then
        local player = mc.player()
        if player then
            local handled = handle_dot_cmd(msg:sub(2), player)
            if handled then return true end
        end
    end

    if module.enabled and module.settings.bridge_out then
        anemoia.zulip_send(msg)
    end

    return false
end)

anemoia.register(module)
