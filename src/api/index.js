// 前端呼叫 Rust 端 Tauri Command 的封裝。所有錯誤統一轉成 ApiError（code + message）。
import { invoke } from '@tauri-apps/api/core'

export class ApiError extends Error {
  constructor(code, message) {
    super(message)
    this.code = code
  }
}

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

function normalize(err) {
  if (err instanceof ApiError) return err
  if (err && typeof err === 'object' && 'code' in err) return new ApiError(err.code, err.message)
  return new ApiError('UNKNOWN', String(err?.message ?? err))
}

async function call(cmd, args) {
  if (!isTauri) {
    throw new ApiError('NOT_TAURI', '請以 WarehouseManager.exe 開啟本程式（瀏覽器無法存取本機檔案）')
  }
  try {
    return await invoke(cmd, args)
  } catch (err) {
    throw normalize(err)
  }
}

export const api = {
  getAppInfo: () => call('get_app_info'),
  getProjects: () => call('get_projects'),
  createProject: (name) => call('create_project', { name }),
  openProject: (project) => call('open_project', { project }),
  readData: (project, fileType) => call('read_data', { project, fileType }),
  writeDataAtomic: (project, fileType, content) => call('write_data_atomic', { project, fileType, content }),
  applyTransaction: (project, tx) => call('apply_transaction', { project, tx }),
  restoreBackup: (project, fileType) => call('restore_backup', { project, fileType }),
  exportTextFile: (defaultName, content) => call('export_text_file', { defaultName, content }),
  exportProjectZip: (project) => call('export_project_zip', { project }),
}
