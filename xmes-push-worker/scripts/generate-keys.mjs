/**
 * Generates a VAPID key pair for use with the xmes-push-worker.
 *
 * Usage: node scripts/generate-keys.mjs
 *
 * Then set the secrets:
 *   wrangler secret put VAPID_PUBLIC_KEY   <- paste the public key
 *   wrangler secret put VAPID_PRIVATE_KEY  <- paste the private key
 *
 * The public key also goes into the PWA's register-sw.js as VAPID_PUBLIC_KEY.
 */

const keyPair = await crypto.subtle.generateKey(
  { name: 'ECDSA', namedCurve: 'P-256' },
  true,
  ['sign', 'verify'],
)

const rawPublic = await crypto.subtle.exportKey('raw', keyPair.publicKey)
const jwkPrivate = await crypto.subtle.exportKey('jwk', keyPair.privateKey)

function b64u(buf) {
  return btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '')
}

const publicKeyB64u = b64u(rawPublic)
const privateKeyB64u = jwkPrivate.d

console.log('VAPID_PUBLIC_KEY  (for wrangler secret + PWA):')
console.log(publicKeyB64u)
console.log()
console.log('VAPID_PRIVATE_KEY (for wrangler secret only — keep secret!):')
console.log(privateKeyB64u)
console.log()
console.log('Run:')
console.log(`  wrangler secret put VAPID_PUBLIC_KEY`)
console.log(`  wrangler secret put VAPID_PRIVATE_KEY`)
