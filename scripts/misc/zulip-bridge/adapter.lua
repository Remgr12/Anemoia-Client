local adapter = {}

-- Basic URL encoding helper
function adapter.url_encode(s)
    return s:gsub(" ", "+"):gsub("([^%w ])", function(c)
        return string.format("%%%02X", string.byte(c))
    end)
end

-- Helper to build Auth header
function adapter.get_auth_header(email, api_key)
    local creds = email .. ":" .. api_key
    return "Basic " .. anemoia.base64_encode(creds)
end

return adapter
