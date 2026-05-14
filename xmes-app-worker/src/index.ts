export default {
  fetch(request: Request): Response {
    const deviceType = request.headers.get('CF-Device-Type');
    const target = deviceType === 'mobile' || deviceType === 'tablet'
      ? 'https://mobile.xmes.org'
      : 'https://desktop.xmes.org';
    return Response.redirect(target, 302);
  },
} satisfies ExportedHandler;
