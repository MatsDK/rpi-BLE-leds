const onButton = document.querySelector(".on")
const offButton = document.querySelector(".off")

onButton.addEventListener("click", () => {
	setLed({ state: "on" })
})

offButton.addEventListener("click", () => {
	setLed({ state: "off" })
})

const setLed = (body) => {
	fetch("/api/set", {
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
	setLed({ state: "color", color: currentColor })
})