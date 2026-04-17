// Register the service worker.
// Note: the SW lives at /assets/sw.js; the server must send the response header
// "Service-Worker-Allowed: /" to allow it to control the root scope.
if ('serviceWorker' in navigator) {
    window.addEventListener('load', function () {
        navigator.serviceWorker
            .register('/assets/sw.js', { scope: '/' })
            .then(function (reg) {
                console.log('[xmes] SW registered, scope:', reg.scope);
            })
            .catch(function (err) {
                console.warn('[xmes] SW registration failed:', err);
            });
    });
}
