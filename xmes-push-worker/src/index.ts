/**
 * xmes-push-worker — Cloudflare Worker
 *
 * Stores Web Push subscriptions per XMTP inbox ID and sends push
 * notifications to group members when a message is sent.
 *
 * Endpoints:
 *   GET  /vapid-public-key          returns VAPID public key for client subscription
 *   POST /subscribe                 register a push subscription
 *   DELETE /subscribe               remove a push subscription
 *   POST /notify                    send push to a list of inbox IDs (called by sender)
 */

import { Hono } from 'hono'
import { cors } from 'hono/cors'

// ── Types ────────────────────────────────────────────────────────────────────

type Env = {
  SUBSCRIPTIONS: KVNamespace
  VAPID_PUBLIC_KEY: string   // base64url uncompressed P-256 public key (65 bytes)
  VAPID_PRIVATE_KEY: string  // base64url raw P-256 private key (32 bytes, JWK "d")
  VAPID_SUBJECT: string      // mailto: or https: contact URI
}

interface PushSubscriptionJSON {
  endpoint: string
  keys: {
    p256dh: string  // base64url client public key
    auth: string    // base64url 16-byte auth secret
  }
}

// ── App ──────────────────────────────────────────────────────────────────────

const app = new Hono<{ Bindings: Env }>()

app.use('*', cors({
  origin: '*',
  allowMethods: ['GET', 'POST', 'DELETE', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Authorization'],
  maxAge: 86400,
}))

// ── GET /vapid-public-key ────────────────────────────────────────────────────

app.get('/vapid-public-key', (c) => {
  return c.json({ publicKey: c.env.VAPID_PUBLIC_KEY })
})

// ── POST /subscribe ──────────────────────────────────────────────────────────

app.post('/subscribe', async (c) => {
  let body: { inbox_id?: string; subscription?: PushSubscriptionJSON }
  try { body = await c.req.json() } catch { return c.json({ error: 'Invalid JSON' }, 400) }

  const { inbox_id, subscription } = body
  if (!inbox_id || !subscription?.endpoint || !subscription?.keys?.p256dh || !subscription?.keys?.auth) {
    return c.json({ error: 'Missing inbox_id or subscription' }, 400)
  }

  await c.env.SUBSCRIPTIONS.put(`sub:${inbox_id}`, JSON.stringify(subscription), {
    expirationTtl: 30 * 24 * 3600, // 30 days
  })

  return c.json({ ok: true })
})

// ── DELETE /subscribe ────────────────────────────────────────────────────────

app.delete('/subscribe', async (c) => {
  let body: { inbox_id?: string }
  try { body = await c.req.json() } catch { return c.json({ error: 'Invalid JSON' }, 400) }

  const { inbox_id } = body
  if (!inbox_id) return c.json({ error: 'Missing inbox_id' }, 400)

  await c.env.SUBSCRIPTIONS.delete(`sub:${inbox_id}`)
  return c.json({ ok: true })
})

// ── POST /notify ─────────────────────────────────────────────────────────────

app.post('/notify', async (c) => {
  let body: { member_inbox_ids?: string[]; sender_inbox_id?: string; group_name?: string; title?: string; body?: string }
  try { body = await c.req.json() } catch { return c.json({ error: 'Invalid JSON' }, 400) }

  const { member_inbox_ids, sender_inbox_id, group_name } = body
  if (!member_inbox_ids || !Array.isArray(member_inbox_ids)) {
    return c.json({ error: 'Missing member_inbox_ids' }, 400)
  }

  // Don't notify the sender
  const targets = member_inbox_ids.filter(id => id !== sender_inbox_id)

  let sent = 0
  let failed = 0

  await Promise.all(targets.map(async (inbox_id) => {
    const raw = await c.env.SUBSCRIPTIONS.get(`sub:${inbox_id}`)
    if (!raw) return

    let subscription: PushSubscriptionJSON
    try { subscription = JSON.parse(raw) } catch { return }

    const payload = JSON.stringify({
      title: body.title ?? group_name ?? 'xmes',
      body:  body.body  ?? 'New message',
    })

    try {
      await sendPush(subscription, payload, c.env)
      sent++
    } catch (err) {
      console.error(`Push failed for ${inbox_id}:`, err)
      // If subscription is gone (410/404), clean it up
      if (err instanceof PushGoneError) {
        await c.env.SUBSCRIPTIONS.delete(`sub:${inbox_id}`)
      }
      failed++
    }
  }))

  return c.json({ ok: true, sent, failed })
})

export default app

// ── Push delivery ─────────────────────────────────────────────────────────────

class PushGoneError extends Error {}

async function sendPush(sub: PushSubscriptionJSON, payload: string, env: Env): Promise<void> {
  const { endpoint } = sub

  const authHeader = await buildVapidHeader(endpoint, env.VAPID_SUBJECT, env.VAPID_PUBLIC_KEY, env.VAPID_PRIVATE_KEY)

  // Encrypt payload per RFC 8291
  const encrypted = await encryptPayload(payload, sub)

  const res = await fetch(endpoint, {
    method: 'POST',
    headers: {
      Authorization: authHeader,
      'Content-Type': 'application/octet-stream',
      'Content-Encoding': 'aes128gcm',
      TTL: '60',
      Urgency: 'high',
      'Content-Length': String(encrypted.byteLength),
    },
    body: encrypted,
  })

  if (res.status === 410 || res.status === 404) throw new PushGoneError()
  if (!res.ok && res.status !== 201) {
    throw new Error(`Push endpoint returned ${res.status}: ${await res.text()}`)
  }
}

// ── VAPID JWT ─────────────────────────────────────────────────────────────────

async function buildVapidHeader(
  endpoint: string,
  subject: string,
  pubKeyB64u: string,
  privKeyB64u: string,
): Promise<string> {
  const audience = new URL(endpoint).origin
  const now = Math.floor(Date.now() / 1000)

  const header  = b64u(te(JSON.stringify({ typ: 'JWT', alg: 'ES256' })))
  const payload = b64u(te(JSON.stringify({ aud: audience, exp: now + 43200, sub: subject })))
  const input   = `${header}.${payload}`

  const signingKey = await importVapidSigningKey(privKeyB64u, pubKeyB64u)
  const sigBytes   = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, signingKey, te(input))

  const token = `${input}.${b64uBytes(new Uint8Array(sigBytes))}`
  return `vapid t=${token},k=${pubKeyB64u}`
}

async function importVapidSigningKey(privKeyB64u: string, pubKeyB64u: string): Promise<CryptoKey> {
  // Public key is 65 bytes uncompressed: 04 || x(32) || y(32)
  const pub = b64uDecode(pubKeyB64u)
  const x = b64uBytes(pub.slice(1, 33))
  const y = b64uBytes(pub.slice(33, 65))

  return crypto.subtle.importKey(
    'jwk',
    { kty: 'EC', crv: 'P-256', d: privKeyB64u, x, y },
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['sign'],
  )
}

// ── RFC 8291 payload encryption ───────────────────────────────────────────────

async function encryptPayload(plaintext: string, sub: PushSubscriptionJSON): Promise<Uint8Array> {
  const salt       = crypto.getRandomValues(new Uint8Array(16))
  const serverKeys = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits'])
  const serverPub  = new Uint8Array(await crypto.subtle.exportKey('raw', serverKeys.publicKey))

  // Recipient public key and auth secret
  const clientPub  = b64uDecode(sub.keys.p256dh)
  const authSecret = b64uDecode(sub.keys.auth)

  // Import client's public key for ECDH
  const clientKey = await crypto.subtle.importKey(
    'raw', clientPub,
    { name: 'ECDH', namedCurve: 'P-256' },
    false, [],
  )

  // ECDH shared secret
  const ecdhBits = new Uint8Array(
    await crypto.subtle.deriveBits({ name: 'ECDH', public: clientKey }, serverKeys.privateKey, 256)
  )

  // HKDF-SHA-256: PRK from auth secret
  const prk = await hkdf(authSecret, ecdhBits, concat(
    te('WebPush: info\x00'), clientPub, serverPub
  ), 32)

  // CEK and nonce via HKDF
  const cek   = await hkdf(salt, prk, te('Content-Encoding: aes128gcm\x00'), 16)
  const nonce = await hkdf(salt, prk, te('Content-Encoding: nonce\x00'), 12)

  // AES-128-GCM encrypt (with padding delimiter byte 0x02)
  const encKey = await crypto.subtle.importKey('raw', cek, { name: 'AES-GCM' }, false, ['encrypt'])
  const padded = concat(te(plaintext), new Uint8Array([0x02]))  // record delimiter

  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, encKey, padded)
  )

  // RFC 8291 §2: header || ciphertext
  // header = salt(16) || rs(4, big-endian) || keyid_len(1) || server_public_key(65)
  const rs = 4096
  const header = new Uint8Array(16 + 4 + 1 + 65)
  header.set(salt, 0)
  new DataView(header.buffer).setUint32(16, rs, false)
  header[20] = 65
  header.set(serverPub, 21)

  return concat(header, ciphertext)
}

async function hkdf(salt: Uint8Array, ikm: Uint8Array, info: Uint8Array, length: number): Promise<Uint8Array> {
  const keyMaterial = await crypto.subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits'])
  const bits = await crypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt, info },
    keyMaterial,
    length * 8,
  )
  return new Uint8Array(bits)
}

// ── Utilities ─────────────────────────────────────────────────────────────────

function te(s: string): Uint8Array {
  return new TextEncoder().encode(s)
}

function concat(...arrays: Uint8Array[]): Uint8Array {
  const total = arrays.reduce((n, a) => n + a.length, 0)
  const out = new Uint8Array(total)
  let offset = 0
  for (const a of arrays) { out.set(a, offset); offset += a.length }
  return out
}

function b64u(data: Uint8Array): string {
  return btoa(String.fromCharCode(...data)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '')
}

function b64uBytes(data: Uint8Array): string {
  return b64u(data)
}

function b64uDecode(s: string): Uint8Array {
  const padded = s.replace(/-/g, '+').replace(/_/g, '/').padEnd(s.length + (4 - s.length % 4) % 4, '=')
  return Uint8Array.from(atob(padded), c => c.charCodeAt(0))
}
