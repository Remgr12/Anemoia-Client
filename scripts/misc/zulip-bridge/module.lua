local adapter = require("scripts.misc.zulip-bridge.adapter")
local ui = require("scripts.misc.zulip-bridge.ui")

local module = {
    name = "ZulipBridge",
    description = "Bridges Zulip chat into Minecraft",
    category = "Misc",
    enabled = false,
    settings = {
        url = "https://zulip.example.com",
        email = "bot@example.com",
        api_key = "secret",
        stream = "general",
        topic = "minecraft",
        poll_rate = 2.0,
        gui_key = 85, -- 'U' key
    },
    _settings_meta = {
        gui_key = { type = "keybind" }
    },
    queue_id = nil,
    last_event_id = -1,
    messages = {},
    last_poll = 0,
    gui_open = false,
}

function module:on_enable()
    self.queue_id = nil
    self.last_event_id = -1
    self.messages = {}
end

function module:on_tick()
    if not self.enabled then return end
    
    local now = os.clock()
    if now - self.last_poll > self.settings.poll_rate then
        self:poll()
        self.last_poll = now
    end

    -- Handle keybind to open/close GUI
    if mc.is_key_down(self.settings.gui_key) then
        if not self._key_was_down then
            self.gui_open = not self.gui_open
            self._key_was_down = true
        end
    else
        self._key_was_down = false
    end
end

function module:poll()
    if not self.queue_id then
        self:register_queue()
        return
    end

    local auth = adapter.get_auth_header(self.settings.email, self.settings.api_key)
    local url = self.settings.url .. "/api/v1/events?queue_id=" .. self.queue_id .. "&last_event_id=" .. self.last_event_id
    
    local res = anemoia.http(url, {
        headers = { Authorization = auth }
    })

    if res.status == 200 and res.json and res.json.result == "success" then
        for _, event in ipairs(res.json.events) do
            if event.type == "message" then
                local msg = event.message
                table.insert(self.messages, {
                    sender = msg.sender_full_name,
                    content = msg.content,
                    time = os.date("%H:%M:%S")
                })
            end
            self.last_event_id = math.max(self.last_event_id, event.id)
        end
    elseif res.status == 400 then
        self.queue_id = nil
    end
end

function module:register_queue()
    local auth = adapter.get_auth_header(self.settings.email, self.settings.api_key)
    local url = self.settings.url .. "/api/v1/register"
    
    local res = anemoia.http(url, {
        method = "POST",
        headers = { 
            Authorization = auth,
            ["Content-Type"] = "application/x-www-form-urlencoded"
        },
        body = "event_types=" .. "[\"message\"]"
    })

    if res.status == 200 and res.json and res.json.result == "success" then
        self.queue_id = res.json.queue_id
        self.last_event_id = res.json.last_event_id
    end
end

function module:send_message(content)
    if content == "" then return end
    local auth = adapter.get_auth_header(self.settings.email, self.settings.api_key)
    local url = self.settings.url .. "/api/v1/messages"
    
    local body = "type=stream&to=" .. adapter.url_encode(self.settings.stream) .. 
                 "&topic=" .. adapter.url_encode(self.settings.topic) .. 
                 "&content=" .. adapter.url_encode(content)
    
    anemoia.http(url, {
        method = "POST",
        headers = { 
            Authorization = auth,
            ["Content-Type"] = "application/x-www-form-urlencoded"
        },
        body = body
    })
end

-- Custom UI rendering inside the "Zulip Bridge" ClickGUI window
anemoia.on_zulip_ui(function(painter)
    ui.draw(painter, module)
end)

anemoia.register(module)
