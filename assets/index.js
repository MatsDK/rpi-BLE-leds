const onButton = document.querySelector(".on")
const offButton = document.querySelector(".off")

let connectedDevice = null;

onButton.addEventListener("click", () => {
	setLed({ event_type: "on" })
})

offButton.addEventListener("click", () => {
	setLed({ event_type: "off" })
})

const setLed = (body) => {
	if (!connectedDevice) return
	fetch(`/api/set/${connectedDevice}`, {
		method: "POST",
		body: JSON.stringify(body),
		headers: {
			"Content-Type": "application/json"
		}
	})
}

const color = document.querySelector(".color");
const setColorButton = document.querySelector(".set_color");

let currentColor = "#ffffff"

color.addEventListener("input", (_) => {
	console.log(color.value);
	currentColor = color.value
});

setColorButton.addEventListener("click", () => {
	setLed({ event_type: "color", color: currentColor })
})

const brightness = document.querySelector(".brightness");
const brightnessButton = document.querySelector(".set_brightness");

let currentBrightness = 255;

brightness.addEventListener("input", (_) => {
	currentBrightness = +brightness.value
});

brightnessButton.addEventListener("click", () => {
	setLed({ event_type: "brightness", brightness: currentBrightness })
})

const devicesList = document.querySelector(".devices_list")

const createDevicesList = () => {
	for (const device of devices) {
		let li = document.createElement("li")
		li.textContent = device

		let button = document.createElement("button")
		button.textContent = "Connect"
		button.addEventListener("click", () => {
			connectTo(device)
		})
		li.appendChild(button)

		devicesList.append(li)
	}
}

createDevicesList()

const connectTo = (addr) => {
	fetch(`/api/connect/${addr}`, {
		method: "POST",
	}).then(res => {
		connectedDevice = addr
	})
}