

function update_online_status() {
    fetch("/wol/online_status").then(res => res.json()).then(res => {
        for (const [device_id, online] of Object.entries(res)) {
            const id = "status-" + device_id;
            const element = document.getElementById(id);
            if (element !== null) {
                element.innerHTML = online ? "🟢" : "🔴"
            }

        }
    })
}

setInterval(update_online_status, 5000);
update_online_status();