(function(W) {
    let editor = null; // gets set by Alpine in response to HTMX afterSettle event
    const ALT_PLACEHOLDER = "image caption";

    function insertImageRef(path) {
        if (editor.disabled) return;
        let insertAt = editor.selectionStart;
        const beforeText = editor.value.substring(0, insertAt);
        const afterText = editor.value.substring(insertAt, editor.value.length);
        let beforeSpacer, afterSpacer;
        if (/\n\n$/.test(beforeText)) {
            beforeSpacer = "";
        } else if (/\n$/.test(beforeText)) {
            beforeSpacer = "\n";
        } else {
            beforeSpacer = "\n\n";
        }
        if (/^\n\n/.test(afterText)) {
            afterSpacer = "";
        } else if (/^\n/.test(afterText)) {
            afterSpacer = "\n";
        } else {
            afterSpacer = "\n\n";
        }
        editor.value =
            `${beforeText}${beforeSpacer}![${ALT_PLACEHOLDER}](${path})${afterSpacer}${afterText}`;
        insertAt += 2 + beforeSpacer.length;
        editor.setSelectionRange(
            insertAt,
            insertAt + ALT_PLACEHOLDER.length,
            "forward",
        );
        editor.focus();
    }

    function getAncestor(node, ancestorNodeName) {
        if (node.nodeName === ancestorNodeName) return node;
        if (node.nodeName === "BODY") return false;
        return getAncestor(node.parentNode, ancestorNodeName);
    }

    function deleteImage(path) {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", () => {
            if (xhr.status === 200) {
                imageList.innerHTML = xhr.response;
                W.location.hash = "";
            } else {
                alert(`Failed to delete image ${path}`);
            }
        });

        path = path.replace(/^/, "/images");
        xhr.open("DELETE", path);
        xhr.send();
    }

    function handleThumbClick(el, shift) {
        if (shift) {
            W.open(el.dataset.path, el.fileName);
        } else {
            insertImageRef(el.dataset.path);
        }
    }

    function imageListClickHandler(event) {
        editor = Alpine.store('editor');
        const el = event.target;
        switch (el.nodeName) {
            case "H4": {
                el.classList.toggle("collapsed");
                break;
            }
            case "BUTTON": {
                const thumb = getAncestor(el, "FIGURE");
                if (
                    thumb &&
                    confirm(
                        `Are you sure you want to delete the image ${thumb.dataset.fileName}?\n\nWarning: this cannot be undone.`,
                    )
                ) {
                    deleteImage(thumb.dataset.path);
                }
                break;
            }
            default: {
                const thumb = getAncestor(el, "FIGURE");
                if (thumb) {
                    handleThumbClick(thumb, event.shiftKey);
                }
                break;
            }
        }
    }

    W.imageListClickHandler = imageListClickHandler;
}(window));
