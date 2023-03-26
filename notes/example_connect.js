// This is the simplified RTC connection setup
// Obviously does not include the signaling server to communicate offer/answer/ice candidates

let local = new RTCPeerConnection({
            iceServers: [
                { urls: 'stun:stun2.l.google.com:19305' }
            ]
        })
let remote = new RTCPeerConnection({
            iceServers: [
                { urls: 'stun:stun2.l.google.com:19305' }
            ]
        })
        
local.onicecandidateerror=(e)=>console.log('local ice error', e)
remote.onicecandidateerror=(e)=>console.log('remote ice error', e)
local.onicecandidate=e=>{
    if (!e.candidate) { console.log('null local candidate', e); return }
    console.log('local candidate', e.candidate);
    remote.addIceCandidate(e.candidate).catch(e => console.error('local add ice error', e))
}
remote.onicecandidate=e=>{
    if (!e.candidate) { console.log('null remote candidate', e); return }
    console.log('remote candidate', e.candidate);
    local.addIceCandidate(e.candidate).catch(e => console.error('remote add ice error', e))
}

let localChannel = local.createDataChannel('testChannel');localChannel.onopen=() => console.log('local open'); local.onclose=()=>console.log('local close')
remote.ondatachannel=({channel})=>{
    channel.onopen=()=>{ console.log('remote open'); localChannel.send('test') }
    channel.onclose=()=>console.log('remote close');
    channel.onmessage=msg=>console.log('recv', msg.data)
}

console.log('Start of offer')

let offer = await local.createOffer()
await local.setLocalDescription(offer)
await remote.setRemoteDescription(offer)

console.log('Start of answer')

let answer = await remote.createAnswer()
await remote.setLocalDescription(answer)
await local.setRemoteDescription(answer)
