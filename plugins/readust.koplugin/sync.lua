local UIManager = require('ui/uimanager')
local logger = require('logger')
local socketutil = require('socketutil')

local SYNC_TIMEOUTS = { 5, 10 }

local ReadustSyncClient = {
  backend_url = nil,
  api_key = nil,
}

function ReadustSyncClient:new(o)
  if o == nil then
    o = {}
  end
  setmetatable(o, self)
  self.__index = self
  if o.init then
    o:init()
  end
  return o
end

function ReadustSyncClient:init()
  local Spore = require('Spore')
  self.client = Spore.new_from_lua({
    base_url = self.backend_url,
    name = 'reqadust-sync-api',
    methods = {
      pull_sync = {
        path = '/sync',
        method = 'GET',
        required_params = {
          'since',
          'type',
          'book',
          'meta_hash',
        },
        expected_status = { 200, 400, 301, 401, 403 },
      },

      push_sync = {
        path = '/sync',
        method = 'POST',
        required_params = { 'books', 'notes', 'configs' },
        payload = { 'books', 'notes', 'configs' },
        expected_status = { 200, 400, 301, 401, 403 },
      },
    },
  })

  -- Readust API headers middleware
  package.loaded['Spore.Middleware.ReadustHeaders'] = {}
  require('Spore.Middleware.ReadustHeaders').call = function(args, req)
    req.headers['content-type'] = 'application/json'
    req.headers['accept'] = 'application/json'
  end

  -- Readust backend url and Bearer token auth middleware
  package.loaded['Spore.Middleware.ReadustAuth'] = {}
  require('Spore.Middleware.ReadustAuth').call = function(args, req)
    if not self.backend_url or self.backend_url == '' then
      logger.err('ReadustSyncClient:backend_url is not set, cannot sync')
      return false, 'Backend URL is required for Readust API'
    end

    if self.api_key then
      req.headers['authorization'] = 'Bearer ' .. self.api_key
    else
      logger.err('ReadustSyncClient:api_key is not set, cannot authenticate')
      return false, 'API key is required for Readust API'
    end
  end

  package.loaded['Spore.Middleware.AsyncHTTP'] = {}
  require('Spore.Middleware.AsyncHTTP').call = function(args, req)
    -- disable async http if Turbo looper is missing
    if not UIManager.looper then
      return
    end
    req:finalize()
    local result
    require('httpclient'):new():request({
      url = req.url,
      method = req.method,
      body = req.env.spore.payload,
      on_headers = function(headers)
        for header, value in pairs(req.headers) do
          if type(header) == 'string' then
            headers:add(header, value)
          end
        end
      end,
    }, function(res)
      result = res
      -- Turbo HTTP client uses code instead of status
      -- change to status so that Spore can understand
      result.status = res.code
      coroutine.resume(args.thread)
    end)
    return coroutine.create(function()
      coroutine.yield(result)
    end)
  end
end

function ReadustSyncClient:pullSync(params, callback)
  logger.dbg('[Call] ReadustSyncClient:pullSync with params ', params)
  self.client:reset_middlewares()
  self.client:enable('Format.JSON')
  self.client:enable('ReadustHeaders', {})
  self.client:enable('ReadustAuth', {})

  socketutil:set_timeout(SYNC_TIMEOUTS[1], SYNC_TIMEOUTS[2])
  local co = coroutine.create(function()
    local ok, res = pcall(function()
      return self.client:pull_sync({
        since = params.since,
        type = params.type,
        book = params.book,
        meta_hash = params.meta_hash,
      })
    end)
    if ok then
      callback(res.status == 200, res.body)
    else
      logger.dbg('ReadustSyncClient:pull_sync failure:', res)
      callback(false, res.body)
    end
  end)
  self.client:enable('AsyncHTTP', { thread = co })
  coroutine.resume(co)
  if UIManager.looper then
    UIManager:setInputTimeout()
  end
  socketutil:reset_timeout()
end

function ReadustSyncClient:pushSync(changes, callback)
  logger.dbg('[Call] ReadustSyncClient:pushSync with changes ', changes)
  self.client:reset_middlewares()
  self.client:enable('Format.JSON')
  self.client:enable('ReadustHeaders', {})
  self.client:enable('ReadustAuth', {})

  socketutil:set_timeout(SYNC_TIMEOUTS[1], SYNC_TIMEOUTS[2])
  local co = coroutine.create(function()
    local ok, res = pcall(function()
      return self.client:push_sync(changes or {})
    end)
    if ok then
      logger.dbg('[Call] ReadustSyncClient:pushSync response code ', res.status, ' data ', res.body)
      callback(res.status == 200, res.body)
    else
      logger.dbg('ReadustSyncClient:push_sync failure:', res)
      callback(false, res.body)
    end
  end)
  self.client:enable('AsyncHTTP', { thread = co })
  coroutine.resume(co)
  if UIManager.looper then
    UIManager:setInputTimeout()
  end
  socketutil:reset_timeout()
end

return ReadustSyncClient
