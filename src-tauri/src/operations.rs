use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

/// 一个正在运行的 Git 子进程及其取消状态。
struct ActiveOperation {
    /// 操作系统进程标识。
    pid: u32,
    /// 前端是否已经请求取消。
    cancelled: Arc<AtomicBool>,
    /// 取消时用于立即唤醒等待中的 Rust 任务。
    cancel_sender: Option<oneshot::Sender<()>>,
    /// Windows Job Object 用于连同 hook 子进程一起终止。
    process_guard: Option<Arc<ProcessGuard>>,
}

/// Windows 上承载 Git 进程树的 Job Object；其他平台使用进程组。
struct ProcessGuard {
    #[cfg(windows)]
    handle: usize,
}

impl ProcessGuard {
    /// 尝试把已启动的 Git 进程加入可整体终止的 Job Object。
    fn attach(pid: u32) -> Option<Self> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };
            use windows_sys::Win32::System::Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
                PROCESS_TERMINATE,
            };

            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }

                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    let _ = CloseHandle(job);
                    return None;
                }

                let process = OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    pid,
                );
                if process.is_null() {
                    let _ = CloseHandle(job);
                    return None;
                }
                let assigned = AssignProcessToJobObject(job, process);
                let _ = CloseHandle(process);
                if assigned == 0 {
                    let _ = CloseHandle(job);
                    return None;
                }

                Some(Self {
                    handle: job as usize,
                })
            }
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            None
        }
    }

    /// 立即终止 Job Object 内的全部进程。
    fn terminate(&self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::JobObjects::TerminateJobObject;

            unsafe { TerminateJobObject(self.handle as _, 1) != 0 }
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Foundation::CloseHandle;

            let _ = CloseHandle(self.handle as _);
        }
    }
}

/// 串行化同一仓库操作，并按 operation ID 跟踪可取消进程。
#[derive(Default)]
pub struct OperationRegistry {
    /// 每个仓库路径对应的串行操作锁。
    repository_locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    /// 以 operation ID 索引的当前子进程。
    active: Mutex<HashMap<String, ActiveOperation>>,
    /// 以 operation ID 索引的可取消异步任务。
    tasks: Mutex<HashMap<String, CancellationToken>>,
}

impl OperationRegistry {
    /// 创建一个没有进行中操作的注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回仓库对应的异步互斥锁，路径使用规范化形式作为键。
    pub async fn repository_lock(&self, repository: &Path) -> Arc<Mutex<()>> {
        let key = repository
            .canonicalize()
            .unwrap_or_else(|_| repository.to_path_buf());
        let mut locks = self.repository_locks.lock().await;
        Arc::clone(locks.entry(key).or_insert_with(|| Arc::new(Mutex::new(()))))
    }

    /// 注册正在运行的 Git 进程，拒绝重复的 operation ID。
    pub async fn register(
        &self,
        operation_id: &str,
        pid: u32,
    ) -> AppResult<(Arc<AtomicBool>, oneshot::Receiver<()>)> {
        if operation_id.trim().is_empty() {
            return Err(AppError::new("GIT_FAILED", "操作标识不能为空"));
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        let (cancel_sender, cancel_receiver) = oneshot::channel();
        let process_guard = ProcessGuard::attach(pid).map(Arc::new);
        let mut active = self.active.lock().await;
        if active.contains_key(operation_id) {
            return Err(AppError::new("GIT_FAILED", "相同操作正在运行"));
        }
        active.insert(
            operation_id.to_string(),
            ActiveOperation {
                pid,
                cancelled: Arc::clone(&cancelled),
                cancel_sender: Some(cancel_sender),
                process_guard,
            },
        );
        Ok((cancelled, cancel_receiver))
    }

    /// 在 Git 进程结束后移除对应的活动记录。
    pub async fn finish(&self, operation_id: &str) {
        self.active.lock().await.remove(operation_id);
    }

    /// 注册可取消的异步任务，例如模型 HTTP 请求。
    pub async fn register_task(&self, operation_id: &str) -> AppResult<CancellationToken> {
        if operation_id.trim().is_empty() {
            return Err(AppError::new("GIT_FAILED", "操作标识不能为空"));
        }
        if self.active.lock().await.contains_key(operation_id) {
            return Err(AppError::new("GIT_FAILED", "相同操作正在运行"));
        }
        let mut tasks = self.tasks.lock().await;
        if tasks.contains_key(operation_id) {
            return Err(AppError::new("GIT_FAILED", "相同操作正在运行"));
        }
        let cancellation = CancellationToken::new();
        tasks.insert(operation_id.to_string(), cancellation.clone());
        Ok(cancellation)
    }

    /// 在异步任务结束后移除取消令牌。
    pub async fn finish_task(&self, operation_id: &str) {
        self.tasks.lock().await.remove(operation_id);
    }

    /// 取消已注册的 Git 进程；进程已结束时返回 `false`。
    pub async fn cancel(&self, operation_id: &str) -> AppResult<bool> {
        let process = {
            let mut active = self.active.lock().await;
            active.get_mut(operation_id).map(|operation| {
                operation.cancelled.store(true, Ordering::SeqCst);
                (
                    operation.pid,
                    operation.cancel_sender.take(),
                    operation.process_guard.clone(),
                )
            })
        };
        let Some((pid, cancel_sender, process_guard)) = process else {
            let task = self.tasks.lock().await.get(operation_id).cloned();
            if let Some(task) = task {
                task.cancel();
                return Ok(true);
            }
            return Ok(false);
        };

        let terminated = process_guard
            .as_ref()
            .is_some_and(|guard| guard.terminate());
        if let Some(sender) = cancel_sender {
            let _ = sender.send(());
        }
        if !terminated {
            terminate_process_tree(pid).await?;
        }
        Ok(true)
    }
}

/// 请求取消指定 operation ID；返回该操作是否仍在运行。
pub async fn cancel_operation(operation_id: &str, registry: &OperationRegistry) -> AppResult<bool> {
    registry.cancel(operation_id).await
}

async fn terminate_process_tree(pid: u32) -> AppResult<()> {
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        crate::child_process::configure_background_command(command.as_std_mut());
        let status = command
            .status()
            .await
            .map_err(|error| AppError::from_io("GIT_FAILED", "无法取消 Git 操作", &error))?;
        let _ = status;
    }

    #[cfg(unix)]
    {
        let mut command = tokio::process::Command::new("kill");
        command
            .args(["-TERM", &format!("-{pid}")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        crate::child_process::configure_background_command(command.as_std_mut());
        let status = command
            .status()
            .await
            .map_err(|error| AppError::from_io("GIT_FAILED", "无法取消 Git 操作", &error))?;
        let _ = status;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cancel_operation, OperationRegistry};

    #[tokio::test]
    async fn cancels_a_registered_async_task() {
        let registry = OperationRegistry::new();
        let cancellation = registry.register_task("model-request").await.unwrap();

        assert!(!cancellation.is_cancelled());
        assert!(cancel_operation("model-request", &registry).await.unwrap());
        assert!(cancellation.is_cancelled());
    }
}
