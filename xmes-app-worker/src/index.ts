const MOBILE_RE = /Android|iPhone|iPad|iPod|webOS|BlackBerry|IEMobile|Opera Mini/i;

export default {
  fetch(request: Request): Response {
    const cfDevice = request.headers.get('CF-Device-Type');
    const ua = request.headers.get('User-Agent') ?? '';
    const isMobile = cfDevice === 'mobile' || cfDevice === 'tablet' || MOBILE_RE.test(ua);
    const target = isMobile ? 'https://mobile.xmes.org' : 'https://desktop.xmes.org';
    return Response.redirect(target, 302);
  },
} satisfies ExportedHandler;
