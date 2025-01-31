const messages = document.getElementById('messages');
const form = document.getElementById('form');
const input = document.getElementById('name');

const clientId = localStorage.getItem('clientId') || generateClientId();
localStorage.setItem('clientId', clientId);

const ws = new WebSocket('wss://cyb3ria.xyz/api/ws');

ws.onopen = () => {
    console.log('WebSocket connection established');
};

ws.onmessage = event => {
    const li = document.createElement('li');
    li.textContent = event.data;
    messages.appendChild(li);
    messages.scrollTop = messages.scrollHeight; // Auto-scroll to the bottom
};

ws.onerror = error => {
    console.error('WebSocket error:', error);
};

ws.onclose = () => {
    console.log('WebSocket connection closed');
};

form.addEventListener('submit', event => {
    event.preventDefault();
    const message = { client_id: clientId, message: input.value };
    ws.send(JSON.stringify(message));
    input.value = '';
});

function generateClientId() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
        const r = Math.random() * 16 | 0, v = c == 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
    });
}
