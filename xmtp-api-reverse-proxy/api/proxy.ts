const UPSTREAMS: Record<string, string> = {
    'xmtp-dev-api.xmes.org':        'https://api.dev.xmtp.network:5558',
    'xmtp-production-api.xmes.org': 'https://api.production.xmtp.network:5558',
};

export default async function handler(req: Request): Promise<Response> {
    const host = (req.headers.get('host') ?? '').split(':')[0];
    const upstream = UPSTREAMS[host];

    if (!upstream) {
        return new Response('Unknown host', { status: 404 });
    }

    const { pathname, search } = new URL(req.url);
    const target = `${upstream}${pathname}${search}`;

    const headers = new Headers(req.headers);
    headers.set('host', new URL(upstream).host);

    const res = await fetch(target, {
        method: req.method,
        headers,
        body: req.method !== 'GET' && req.method !== 'HEAD' ? req.body : undefined,
        // @ts-ignore
        duplex: 'half',
    });

    return new Response(res.body, {
        status:     res.status,
        statusText: res.statusText,
        headers:    res.headers,
    });
}
