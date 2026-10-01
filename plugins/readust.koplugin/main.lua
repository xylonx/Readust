local Dispatcher = require('dispatcher')
local InfoMessage = require('ui/widget/infomessage')
local MultiInputDialog = require('ui/widget/multiinputdialog')
local WidgetContainer = require('ui/widget/container/widgetcontainer')
local NetworkMgr = require('ui/network/manager')
local UIManager = require('ui/uimanager')
local logger = require('logger')
local sha2 = require('ffi/sha2')
local T = require('ffi/util').template
local _ = require('gettext')

local SyncConfig = require('syncconfig')
local SyncAnnotations = require('syncannotations')

local ReadustSync = WidgetContainer:new({
  name = 'readust',
  title = _('Readust Sync'),
  settings = nil,
})

local API_CALL_DEBOUNCE_DELAY = 0
local AP_CALL_FOR_PAGE_UPDATE = 0

ReadustSync.default_settings = {
  backend_url = '',
  api_key = '',
  auto_sync = false,
  last_sync_at = nil,
}

-- ── Lifecycle ──────────────────────────────────────────────────────

function ReadustSync:init()
  self.lasy_sync_timestamp = 0
  self.settings = G_reader_settings:readSetting('readust_sync', self.default_settings)

  local meta = dofile(self.path .. '/_meta.lua')
  self.installed_version = meta and meta.version and tostring(meta.version)

  self.ui.menu:registerToMainMenu(self)
end

function ReadustSync:onDispatcherRegisterActions()
  Dispatcher:registerAction('readust_sync_set_auto_sync', {
    category = 'string',
    event = 'ReadustSyncToggleAutoSync',
    title = _('Set auto progress sync'),
    reader = true,
    args = { true, false },
    toggle = { _('on'), _('off') },
  })
  Dispatcher:registerAction(
    'readust_sync_toggle_autosync',
    { category = 'none', event = 'ReadustSyncToggleAutoSync', title = _('Toggle auto readust sync'), reader = true }
  )

  Dispatcher:registerAction('readust_sync_push_progress', {
    category = 'none',
    event = 'ReadustSyncPushProgress',
    title = _('Push readust progress from this device'),
    reader = true,
  })
  Dispatcher:registerAction('readust_sync_pull_progress', {
    category = 'none',
    event = 'ReadustSyncPullProgress',
    title = _('Pull readust progress from other devices'),
    reader = true,
    separator = true,
  })
  Dispatcher:registerAction('readust_sync_push_annotations', {
    category = 'none',
    event = 'ReadustSyncPushAnnotations',
    title = _('Push readust annotations from this device'),
    reader = true,
  })
  Dispatcher:registerAction('readust_sync_pull_annotations', {
    category = 'none',
    event = 'ReadustSyncPullAnnotations',
    title = _('Pull readust annotations from other devices'),
    reader = true,
    separator = true,
  })
end

function ReadustSync:onReaderReady()
  if self.settings.auto_sync and self.settings.backend_url and self.settings.api_key then
    UIManager:nextTick(function()
      self:pullBookConfig(false)
      self:pullBookNotes(false)
    end)
  end
  self:onDispatcherRegisterActions()
end

-- ── Menu ───────────────────────────────────────────────────────────

function ReadustSync:setupBackend(settings, menu)
  local dialog
  dialog = MultiInputDialog:new({
    title = 'Setup Readust backend',
    fields = {
      {
        text = settings.backend_url,
        hint = 'Backend URL',
      },
      {
        text = settings.api_key,
        hint = 'API Key',
      },
    },
    buttons = {
      {
        {
          text = _('Cancel'),
          id = 'close',
          callback = function()
            UIManager:close(dialog)
          end,
        },
        {
          text = _('Confirm'),
          callback = function()
            local backend_url, api_key = unpack(dialog:getFields())
            if backend_url == '' or api_key == '' then
              UIManager:show(InfoMessage:new({
                text = _('Please enter both backend_url and api_key'),
                timeout = 2,
              }))
              return
            end
            UIManager:close(dialog)
            settings.backend_url = backend_url
            settings.api_key = api_key
            G_reader_settings:saveSetting('readust_sync', settings)

            if menu then
              menu:updateItems()
            end

            UIManager:show(InfoMessage:new({
              text = _('Successfully set readust endpoint'),
              timeout = 3,
            }))
          end,
        },
      },
    },
  })

  UIManager:show(dialog)
  dialog:onShowKeyboard()
end

function ReadustSync:addToMainMenu(menu_items)
  menu_items.readust_sync = {
    sorting_hint = 'tools',
    text = _('Readust Sync'),
    sub_item_table = {
      {
        text_func = function()
          if self.settings.backend_url == '' or self.settings.api_key == '' then
            return _('Setup backend')
          else
            return self.settings.backend_url
          end
        end,
        callback_func = function()
          return function(menu)
            self:setupBackend(self.settings, menu)
          end
        end,
        separator = true,
      },
      {
        text = _('Auto sync progress and annotations'),
        checked_func = function()
          return self.settings.auto_sync
        end,
        callback = function()
          self:onReadustSyncToggleAutoSync()
        end,
        separator = true,
      },
      {
        text = _('Push book config now'),
        enabled_func = function()
          return self.settings.backend_url ~= nil and self.settings.api_key ~= nil and self.ui.document ~= nil
        end,
        callback = function()
          self:onReadustSyncPushProgress()
        end,
      },
      {
        text = _('Pull book config now'),
        enabled_func = function()
          return self.settings.backend_url ~= nil and self.settings.api_key ~= nil and self.ui.document ~= nil
        end,
        callback = function()
          self:onReadustSyncPullProgress()
        end,
        separator = true,
      },
      {
        text = _('Push annotations now'),
        enabled_func = function()
          return self.settings.backend_url ~= nil and self.settings.api_key ~= nil and self.ui.document ~= nil
        end,
        callback = function()
          self:onReadustSyncPushAnnotations()
        end,
      },
      {
        text = _('Pull annotations now'),
        enabled_func = function()
          return self.settings.backend_url ~= nil and self.settings.api_key ~= nil and self.ui.document ~= nil
        end,
        callback = function()
          self:onReadustSyncPullAnnotations()
        end,
      },
      {
        text = _('Full sync all annotations'),
        enabled_func = function()
          return self.settings.backend_url ~= nil and self.settings.api_key ~= nil and self.ui.document ~= nil
        end,
        callback = function()
          self:fullSyncBookNotes()
        end,
        separator = true,
      },
    },
  }
end

-- ── Sync helpers (thin wrappers around modules) ────────────────────
function ReadustSync:ensureClient(interactive)
  if not self.settings.backend_url or not self.settings.api_key then
    return nil
  end

  local ReadustSyncClient = require('sync')
  return ReadustSyncClient:new({
    backend_url = self.settings.backend_url,
    api_key = self.settings.api_key,
  })
end

function ReadustSync:getBookIdentifiers()
  local book_hash = SyncConfig:getDocumentIdentifier(self.ui)
  local meta_hash = SyncConfig:getMetaHash(self.ui)
  return book_hash, meta_hash
end

-- ── Config sync ────────────────────────────────────────────────────

function ReadustSync:pushBookConfig(interactive)
  local now = os.time()

  if not interactive and now - self.last_sync_timestamp <= API_CALL_DEBOUNCE_DELAY then
    return
  end

  if interactive and NetworkMgr:willRerunWhenOnline(function()
    self:pushBookConfig(interactive)
  end) then
    return
  end

  local client = self:ensureClient(interactive)
  if not client then
    return
  end

  self.last_sync_timestamp = SyncConfig:push(self.ui, self.settings, client, interactive, self.last_sync_timestamp)
end

function ReadustSync:pullBookConfig(interactive)
  local book_hash, meta_hash = self:getBookIdentifiers()
  if not book_hash or not meta_hash then
    return
  end

  if NetworkMgr:willRerunWhenOnline(function()
    self:pullBookConfig(interactive)
  end) then
    return
  end

  local client = self:ensureClient(interactive)
  if not client then
    return
  end

  SyncConfig:pull(self.ui, self.settings, client, book_hash, meta_hash, interactive)
end

-- ── Annotation sync ────────────────────────────────────────────────

function ReadustSync:pushBookNotes(interactive, full_sync)
  if
    interactive and NetworkMgr:willRerunWhenOnline(function()
      self:pushBookNotes(interactive, full_sync)
    end)
  then
    return
  end

  local client = self:ensureClient(interactive)
  if not client then
    return
  end

  SyncAnnotations:push(self.ui, self.settings, client, interactive, full_sync)
end

function ReadustSync:pullBookNotes(interactive, full_sync)
  local book_hash, meta_hash = self:getBookIdentifiers()
  if not book_hash or not meta_hash then
    return
  end

  if NetworkMgr:willRerunWhenOnline(function()
    self:pullBookNotes(interactive, full_sync)
  end) then
    return
  end

  local client = self:ensureClient(interactive)
  if not client then
    return
  end

  SyncAnnotations:pull(self.ui, self.settings, client, book_hash, meta_hash, self.dialog, interactive, full_sync)
end

function ReadustSync:fullSyncBookNotes()
  -- Push all annotations first, then pull all
  self:pushBookNotes(true, true)
  self:pullBookNotes(true, true)
end

-- ── Event handlers ─────────────────────────────────────────────────

function ReadustSync:onReadustSyncToggleAutoSync(toggle)
  if toggle == self.settings.auto_sync then
    return true
  end
  self.settings.auto_sync = not self.settings.auto_sync
  G_reader_settings:saveSetting('readust_sync', self.settings)
  if self.settings.auto_sync and self.ui.document then
    self:pullBookConfig(false)
  end
end

function ReadustSync:onReadustSyncPushProgress()
  self:pushBookConfig(true)
end

function ReadustSync:onReadustSyncPullProgress()
  self:pullBookConfig(true)
end

function ReadustSync:onReadustSyncPushAnnotations()
  self:pushBookNotes(true)
end

function ReadustSync:onReadustSyncPullAnnotations()
  self:pullBookNotes(true)
end

function ReadustSync:onCloseDocument()
  if self.settings.auto_sync and self.settings.backend_url and self.settings.api_key  then
    NetworkMgr:goOnlineToRun(function()
      self:pushBookConfig(false)
      self:pushBookNotes(false)
    end)
  end
end

function ReadustSync:onPageUpdate(page)
  if self.settings.auto_sync and self.settings.api_key and page then
    if self.delayed_push_task then
      UIManager:unschedule(self.delayed_push_task)
    end
    self.delayed_push_task = function()
      self:pushBookConfig(false)
    end
    UIManager:scheduleIn(AP_CALL_FOR_PAGE_UPDATE, self.delayed_push_task)
  end
end

function ReadustSync:onAnnotationsModified()
  if self.settings.auto_sync and self.settings.access_token then
    UIManager:nextTick(function()
      self:pushBookNotes(false)
    end)
  end
end

function ReadustSync:onCloseWidget()
  if self.delayed_push_task then
    UIManager:unschedule(self.delayed_push_task)
    self.delayed_push_task = nil
  end
end

return ReadustSync
