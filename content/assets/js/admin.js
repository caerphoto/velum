(function(D) {
    function $(selector) {
        return D.querySelector(selector);
    }
    function $$(selector) {
        return Array.from(D.querySelectorAll(selector));
    }

    const saveForm = $("#save-article");
    const articleList = $("#admin-article-manager ol");
    const createNew = $("#admin-new-article");
    const editorSection = $("#admin-article-editor");
    const editor = $("#article-editor-input");
    const saveBtn = $("#save-article button");
    const successMsg = $("#save-success");
    const listSectionTabContaner = $("#admin-list-sections-tabs");
    const listSectionTabs = $$("#admin-list-sections-tabs li");
    const listSections = $$('.tab-content[data-tab-set="admin-list-section"]');

    const imageList = $("#admin-image-list");
    const imageUploadForm = $("#image-upload-form");
    const thumbsProgress = $("#thumbs-progress");
    const thumbsProgressBar = $("#thumbs-progress-bar");
    const thumbsProgressCompleted = $("#thumbs-progress-completed");
    const thumbsProgressTotal = $("#thumbs-progress-total");

    let intervalID;

    const ALT_PLACEHOLDER = "image caption";

    function fetchArticleText(slug) {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", () => {
            editor.value = xhr.responseText;
            editorSection.classList.remove("new");
            saveForm.dataset.slug = slug;
            saveForm.dataset.method = "PUT";
            window.location.hash = slug;
            editor.disabled = false;
        });
        xhr.open("GET", `/article/${slug}/text`);
        xhr.send();
    }

    function confirmDelete(slug, title) {
        if (
            !window.confirm(
                `"${title}"\n\nAre you sure you want to delete this article?\n\nWARNING: this cannot be undone!`,
            )
        ) {
            return;
        }

        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", () => {
            const li = articleList.querySelector(`li[data-slug="${slug}"]`);
            if (!li) return;
            li.parentNode.removeChild(li);
            editor.value = "";
        });
        xhr.open("DELETE", `/article/${slug}`);
        xhr.send();
    }

    articleList.addEventListener("click", (event) => {
        const target = event.target;
        if (target.nodeName === "A") {
            if (!target.dataset.slug) return;
            fetchArticleText(target.dataset.slug);
        } else if (target.nodeName === "BUTTON") {
            confirmDelete(
                target.dataset.slug,
                target.dataset.title,
            );
        }
    });

    createNew.addEventListener("submit", (event) => {
        event.preventDefault();
        editor.value = "";
        editorSection.classList.add("new");
        saveForm.dataset.method = "POST";
        delete saveForm.dataset.slug;
        window.location.hash = "";
        editor.disabled = false;
        editor.focus();
    });

    function insertNewArticle(listItemHtml) {
        if (!editorSection.classList.contains("new")) return;

        editorSection.classList.remove("new");
        const frag = D.createElement("template");
        frag.innerHTML = listItemHtml;
        const newLi = frag.content.querySelector("li");
        const slug = newLi.dataset.slug;
        fetchArticleText(slug);

        newLi.classList.add("new");
        articleList.prepend(frag.content);
        setTimeout(() => {
            newLi.classList.remove("new");
        }, 2000);
    }

    saveForm.addEventListener("submit", (event) => {
        event.preventDefault();

        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", function() {
            saveBtn.disabled = false;
            saveBtn.textContent = "Save";

            if (this.status === 200) {
                successMsg.classList.add("visible");
                setTimeout(() => {
                    successMsg.classList.remove("visible");
                }, 2000);
                insertNewArticle(this.response);
            } else {
                // TODO: custom error popup
                alert("Error saving article");
            }
        });

        const path = saveForm.dataset.method === "POST"
            ? "/articles"
            : `/article/${saveForm.dataset.slug}`;
        xhr.open(saveForm.dataset.method, path);
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.send(editor.value);

        saveBtn.disabled = true;
        saveBtn.textContent = "...";
    });

    if (window.location.hash) {
        const slug = window.location.hash.replace(/^#/, "");
        fetchArticleText(slug);
    }

    function activateTabContent(sectionId) {
        listSections.forEach((s) => s.classList.remove("active"));
        D.querySelector(sectionId).classList.add("active");
    }

    listSectionTabContaner.addEventListener("click", (event) => {
        if (event.target.nodeName !== "A") return;
        event.preventDefault();

        activateTabContent(event.target.getAttribute("href"));
        listSectionTabs.forEach((t) => t.classList.remove("active"));
        event.target.parentNode.classList.add("active");
    });

    function setImageListLoading() {
        imageList.innerHTML = "<li>Fetching thumbnails&hellip;</li>";
        imageList.classList.add("loading");
        imageUploadForm.classList.add("disabled");
    }

    function setImageListUploading() {
        imageList.innerHTML = "<li>Uploading images&hellip;</li>";
        imageList.classList.add("uploading");
        imageUploadForm.classList.add("disabled");
    }

    function fetchThumbCounts(cb) {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", () => {
            cb(JSON.parse(xhr.response));
        });
        xhr.open("GET", imageList.dataset.countsPath);
        xhr.send();
    }

    function handleImageListResponse() {
        imageList.classList.remove("loading");
        imageList.classList.remove("uploading");
        imageUploadForm.classList.remove("disabled");
        if (this.status === 200) {
            imageList.innerHTML = this.response;
            const listEl = imageList.querySelector("ul");
            const total = parseInt(listEl.dataset.initialCount, 10);
            const count = parseInt(listEl.dataset.remaining, 10);
            const done = total - count;
            if (count > 0) {
                thumbsProgress.classList.add("active");
                thumbsProgressBar.max = total;
                thumbsProgressBar.value = done;

                thumbsProgressCompleted.textContent = done;
                thumbsProgressTotal.textContent = total;

                clearInterval(intervalID);
                intervalID = setInterval(() => {
                    fetchThumbCounts((counts) => {
                        if (counts.count === 0) {
                            thumbsProgress.classList.remove("active");
                            clearInterval(intervalID);
                            loadImageList();
                        } else {
                            const done = counts.total - counts.count;
                            thumbsProgressBar.value = done;
                            thumbsProgressCompleted.textContent = done;
                            thumbsProgressTotal.textContent = counts.total;
                        }
                    });
                }, 1000);
            } else {
                thumbsProgress.classList.remove("active");
            }
        } else {
            imageList.innerHTML = this.response;
        }
    }

    function loadImageList() {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener("load", handleImageListResponse);
        xhr.open("GET", imageList.dataset.source);
        xhr.send();
        setImageListLoading();
    }

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
                window.location.hash = "";
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
            window.open(el.dataset.path, el.fileName);
        } else {
            insertImageRef(el.dataset.path);
        }
    }

    imageList.addEventListener("click", (event) => {
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
    });

    imageUploadForm.addEventListener("submit", (event) => {
        event.preventDefault();
        const xhr = new XMLHttpRequest();
        const data = new FormData(imageUploadForm);
        xhr.addEventListener("load", handleImageListResponse);
        xhr.open(imageUploadForm.method, imageUploadForm.action);
        xhr.send(data);
        setImageListUploading();
    });

    activateTabContent($(".tab.active a").getAttribute("href"));
    loadImageList();
})(window.document);
