import { isTauri, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
export async function setupTaskLifecycle(): Promise<void> {
  if (!isTauri()) return;
  await listen('timer-quit-requested', () => {
    if (document.querySelector('.timer-quit')) return;
    const dialog = document.createElement('dialog');
    dialog.className = 'timer-quit';
    dialog.setAttribute('aria-label', '退出时如何处理运行任务');
    dialog.innerHTML =
      '<h2>仍有任务正在运行</h2><p>彻底退出会停止后续调度。可以继续后台运行、等待当前任务完成后退出，或停止受管任务后退出。已打开的独立应用不会被关闭。</p><div class="timer-actions"><button class="button" data-quit="continue">继续后台运行</button><button class="button" data-quit="wait">等待完成后退出</button><button class="button danger" data-quit="stop">停止任务后退出</button></div><p role="status"></p>';
    document.body.append(dialog);
    dialog.showModal();
    dialog.addEventListener('click', (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>('[data-quit]');
      if (!button) return;
      const mode = button.dataset.quit;
      void invoke('timer_shutdown', { mode })
        .then(() => {
          dialog.close();
          dialog.remove();
        })
        .catch((error) => {
          dialog.querySelector('[role="status"]')!.textContent = String(error);
        });
    });
    dialog.addEventListener('cancel', () => dialog.remove());
  });
}
