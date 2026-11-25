#!/usr/bin/env node
import { createECDH } from 'node:crypto';

// Generate an uncompressed P-256 ECDH/ECDSA keypair formatted as base64url
// (no padding) for RFC 8292 Web Push VAPID configuration.
const ecdh = createECDH('prime256v1');
ecdh.generateKeys();

const publicKey = ecdh.getPublicKey(undefined, 'uncompressed').toString('base64url');
const privateKey = ecdh.getPrivateKey().toString('base64url');

console.log('# Paste into .env (local) and /opt/payroll/secrets.env (production)');
console.log(`VAPID_PUBLIC_KEY=${publicKey}`);
console.log(`VAPID_PRIVATE_KEY=${privateKey}`);
console.log('VAPID_SUBJECT=mailto:support@payrollmy.com');
