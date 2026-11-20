document.addEventListener('click', event => {
  const button = event.target.closest('[data-demo]');
  if (button) document.querySelector('#demo-status').textContent = `已点击「${button.dataset.demo}」组件样例，仅展示反馈。`;
});
