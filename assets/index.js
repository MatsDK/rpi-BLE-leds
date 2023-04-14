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