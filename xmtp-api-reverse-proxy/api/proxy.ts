import type { VercelRequest, VercelResponse } from '@vercel/node';
import { Buffer } from 'node:buffer';

const UPSTREAMS: Record<string, string> = {
    'xmtp-dev-api.xmes.org':        'https://api.dev.xmtp.network:5558',
    'xmtp-production-api.xmes.org': 'https://api.production.xmtp.network:5558',
};

export default async function handler(req: VercelRequest, res: VercelResponse) {
    const host = (req.headers['host'] ?? '').split(':')[0];
    const upstream = UPSTREAMS[host];

    if (!upstream) {
        return res.status(404).send('Unknown host');
    }

    const target = `${upstream}${req.url}`;

    const headers: Record<string, string> = {};
    for (const [key, value] of Object.entries(req.headers)) {
        if (value) headers[key] = Array.isArray(value) ? value[0] : value;
    }
    headers['host'] = new URL(upstream).host;

    const response = await fetch(target, {
        method: req.method,
        headers,
        body: req.method !== 'GET' && req.method !== 'HEAD' ? req : undefined,
        // @ts-ignore
        duplex: 'half',
    });

    res.status(response.status);
    response.headers.forEach((value, key) => res.setHeader(key, value));
    const buffer = await response.arrayBuffer();
    res.send(Buffer.from(buffer));
}