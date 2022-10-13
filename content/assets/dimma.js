/*eslint indent: ["warn", 4], quotes: ["error", "double"]*/
/* Sets 'dimmed' class on page body and 'focus' class on an image when clicked.
 * Also scrolls clicked image fully into view.
 * */
(function (D) {
    const B = D.body;
    const elContent = D.querySelector(".post-content");
    let currentFocus;

    if (!elContent) return;

    elContent.addEventListener("click", function (evt) {
        if (evt.target.nodeName !== "IMG") {
            return;
        }

        if (currentFocus) {
            currentFocus.classList.remove("focus");
        }
        currentFocus = evt.target;
        currentFocus.classList.remove("focus");
        B.classList.add("dimmed");

        currentFocus.parentNode.scrollIntoView({ behavior: "smooth", block: "center" });
        // W.scrollBy(0, -5);
    }, false);

    B.addEventListener("click", function (evt) {
        if (!currentFocus || evt.target === currentFocus) {
            return;
        }
        currentFocus.classList.remove("focus");
        B.classList.remove("dimmed");
    }, false);
}(document, window));
