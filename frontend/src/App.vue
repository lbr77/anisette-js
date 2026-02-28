<script setup>
import { ref } from 'vue'
import { Anisette, loadWasmModule } from 'anisette-js'
import { initLibcurl } from './libcurl-init'
import { LibcurlHttpClient } from './libcurl-http'

const status = ref('Ready')
const headers = ref(null)

async function runDemo() {
  try {
    status.value = 'Loading libcurl...'
    await initLibcurl()
    const httpClient = new LibcurlHttpClient()

    status.value = 'Loading WASM...'
    const wasmModule = await loadWasmModule({ printErr: () => {}})

    status.value = 'Loading library files...'
    const [ssResp, caResp] = await Promise.all([
      fetch('/libstoreservicescore.so'),
      fetch('/libCoreADI.so')
    ])

    if (!ssResp.ok || !caResp.ok) {
      throw new Error('Failed to load .so files from public directory')
    }

    const ssBytes = new Uint8Array(await ssResp.arrayBuffer())
    const caBytes = new Uint8Array(await caResp.arrayBuffer())

    status.value = 'Initializing Anisette...'
    const anisette = await Anisette.fromSo(ssBytes, caBytes, wasmModule, {
      httpClient,
      init: { libraryPath: '/anisette' }
    })
    console.log(anisette.getDevice())
    if (!anisette.isProvisioned) {
      status.value = 'Provisioning...'
      await anisette.provision()
    }

    status.value = 'Getting headers...'
    headers.value = await anisette.getData()
    status.value = 'Done'
  } catch (err) {
    status.value = `Error: ${err.message}`
    console.error(err)
  }
}
</script>

<template>
  <div>
    <h1>Anisette JS Demo</h1>

    <div>
      <button @click="runDemo">Run</button>
    </div>

    <p>Status: {{ status }}</p>

    <pre v-if="headers">{{ JSON.stringify(headers, null, 2) }}</pre>
  </div>
</template>