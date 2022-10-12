/*eslint indent: ["warn", 4], quotes: ["error", "double"]*/
/* Sets 'dimmed' class on page body and 'focus' class on an image when clicked.
 * Also scrolls clicked image fully into view.
 * */
(function (D) {
    var B = D.body;
    var elContent = D.querySelector(".post-content");
    var currentFocus;

    if (!elContent) return;

    function addClass(el, className) {
        var classes = el.className.split(/\s+/);

        classes = classes.filter(function (c) {
            return c.length > 0;
        });

        if (classes.indexOf(className) === -1) {
            classes.push(className);
        }

        el.className = classes.join(" ");
    }

    function removeClass(el, className) {
        var classes = el.className.split(/\s+/);
        var index;
        var rxOnlySpaces = /^\s+$/;

        classes = classes.filter(function (c) {
            return c.length > 0 && !rxOnlySpaces.test(c);
        });

        index = classes.indexOf(className);
        if (index !== -1) {
            classes.splice(index, 1);
        }

        el.className = classes.join(" ");
    }

    elContent.addEventListener("click", function (evt) {
        if (evt.target.nodeName !== "IMG") {
            return;
        }

        if (currentFocus) {
            removeClass(currentFocus, "focus");
        }
        currentFocus = evt.target;
        addClass(currentFocus, "focus");
        addClass(B, "dimmed");

        currentFocus.parentNode.scrollIntoView({ behavior: "smooth", block: "center" });
        // W.scrollBy(0, -5);
    }, false);

    B.addEventListener("click", function (evt) {
        if (!currentFocus || evt.target === currentFocus) {
            return;
        }
        removeClass(currentFocus, "focus");
        removeClass(B, "dimmed");
    }, false);
}(document, window));
