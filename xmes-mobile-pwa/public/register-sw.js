if ('serviceWorker' in navigator) {
    window.addEventListener('load', function () {
        navigator.serviceWorker
            .register('/sw.js')
            .then(function (reg) {
                console.log('[xmes] SW registered, scope:', reg.scope);
            })
            .catch(function (err) {
                console.warn('[xmes] SW registration failed:', err);
            });
    });
}
