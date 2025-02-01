// Получаем IP-адрес клиента
let ipAddress = '';
fetch('https://api.ipify.org?format=json')
    .then(response => response.json())
    .then(data => {
        ipAddress = data.ip;
        console.log('Client IP Address:', ipAddress);
    })
    .catch(error => {
        console.error('Error fetching IP address:', error);
    });

// MAC-адрес нельзя получить напрямую в браузере, но можно использовать заглушку
let macAddress = '00:00:00:00:00:00'; // Заглушка для MAC-адреса

const messages = document.getElementById('messages');
const form = document.getElementById('form');
const input = document.getElementById('name');

const urlParams = new URLSearchParams(window.location.search);
const username = urlParams.get('username');

let ws = null;

function connectWebSocket() {
  if (ws) {
      ws.close();
      console.log('WebSocket connection closed');
   }
    ws = new WebSocket(`wss://cyb3ria.xyz/api/ws?username=${encodeURIComponent(username)}`);

    ws.onopen = () => {
        console.log('WebSocket connection established');
         document.getElementById('connection-status').textContent = "Connected";
    };

    ws.onmessage = event => {
        const li = document.createElement('li');
        li.textContent = event.data;
        messages.appendChild(li);
        messages.scrollTop = messages.scrollHeight; // Auto-scroll to the bottom
    };

    ws.onerror = error => {
        console.error('WebSocket error:', error);
         document.getElementById('connection-status').textContent = "Error";
        setTimeout(connectWebSocket, 5000) // Попытка переподключения
    };

    ws.onclose = () => {
        console.log('WebSocket connection closed');
        document.getElementById('connection-status').textContent = "Disconnected";
         setTimeout(connectWebSocket, 5000) // Попытка переподключения
    };
}

if (messages) {
    connectWebSocket();
    form.addEventListener('submit', event => {
        event.preventDefault();
        const message = {
            username: username,
            message: input.value,
            ip: ipAddress,
            mac: macAddress
        };
        ws.send(JSON.stringify(message));
        input.value = '';
    });
}