(() => {
  try {
    const value = localStorage.getItem('carddue-theme') || 'system';
    const dark = value === 'dark' || (value === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  } catch { /* System colors remain available when storage is disabled. */ }
})();
